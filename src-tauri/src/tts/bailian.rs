//! Native Qwen-Audio-TTS HTTP protocol (not OpenAI audio/speech).
use anyhow::{bail, Context};
use base64::Engine;
use reqwest::Client;
use serde_json::{json, Value};

const ENDPOINT: &str = "/api/v1/services/audio/tts/SpeechSynthesizer";

pub(super) fn matches(base_url: &str, model: &str) -> bool {
    model.starts_with("qwen-audio-") || base_url.trim_end_matches(['#', '/']).ends_with(ENDPOINT)
}

fn endpoint(base_url: &str) -> anyhow::Result<String> {
    let base = base_url.trim();
    let raw = base.ends_with('#');
    let mut url = reqwest::Url::parse(base.trim_end_matches('#')).context("百炼 API 地址无效")?;
    if !matches!(url.scheme(), "https" | "http") {
        bail!("百炼 API 地址必须使用 HTTP 或 HTTPS");
    }
    if !raw {
        match url.path().trim_end_matches('/') {
            "" | "/" | "/api" | "/api/v1" | "/v1" => url.set_path(ENDPOINT),
            path if path == ENDPOINT => url.set_path(ENDPOINT),
            _ => bail!("百炼请填写工作空间根地址、/api/v1 或完整 SpeechSynthesizer 端点"),
        }
    }
    Ok(url.into())
}

fn request_body(model: &str, text: &str, extra: &str) -> anyhow::Result<Value> {
    let extra: Value = if extra.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(extra).context("百炼自定义参数不是有效 JSON")?
    };
    let params = extra
        .as_object()
        .context("百炼自定义参数必须是 JSON 对象")?;
    let mut input = json!({
        "text": text,
        "voice": "longanhuan_v3.1",
        "format": "wav",
        "sample_rate": 24000
    });
    // Deliberately isolate this protocol from inherited SiliconFlow/MiMo parameters.
    // Accept both the settings editor's flat fields and the native input object.
    let nested = params
        .get("input")
        .map(|v| v.as_object().context("input 必须是对象"))
        .transpose()?;
    for source in [Some(params), nested].into_iter().flatten() {
        for key in [
            "voice",
            "format",
            "sample_rate",
            "language_hints",
            "instruction",
            "volume",
            "rate",
            "pitch",
            "seed",
            "enable_ssml",
            "hot_fix",
        ] {
            if let Some(value) = source.get(key) {
                input[key] = value.clone();
            }
        }
    }
    let voice = input["voice"].as_str().context("百炼 voice 必须是字符串")?;
    if voice.is_empty() || voice.contains('/') || voice.contains(':') {
        bail!("百炼请使用裸音色名，如 longanhuan_v3.1；不要继承 SiliconFlow 的模型:音色");
    }
    if !matches!(input["format"].as_str(), Some("wav" | "mp3")) {
        bail!("百炼本地播放支持 wav / mp3，请使用其中一种格式");
    }
    if let Some(hints) = input.get("language_hints") {
        let hints = hints
            .as_array()
            .context("language_hints 必须是数组，如 [\"ja\"]")?;
        if hints.is_empty() || hints.iter().any(|v| v.as_str().is_none_or(str::is_empty)) {
            bail!("language_hints 必须包含非空语言代码，如 [\"ja\"]");
        }
    }
    Ok(json!({"model": model, "input": input}))
}

pub(super) async fn synthesize(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    extra: &str,
    text: &str,
) -> anyhow::Result<String> {
    if api_key.trim().is_empty() {
        bail!("百炼未配置 API 密钥，请在 TTS 提供商中填写同地域的百炼 API Key");
    }
    let url = endpoint(base_url)?;
    let body = request_body(model, text, extra)?;
    log::info!("[TTS] 协议=百炼, model={}, 文本长度={}", model, text.len());
    let response = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .timeout(super::WHOLE_REQUEST_TIMEOUT)
        .send()
        .await?;
    let status = response.status();
    let value: Value = response.json().await.context("百炼返回的不是 JSON")?;
    if !status.is_success() || value.get("code").is_some_and(|v| !v.is_null()) {
        bail!(
            "百炼 TTS 错误 ({}), code={}, message={}",
            status,
            value["code"].as_str().unwrap_or("unknown"),
            value["message"].as_str().unwrap_or("合成失败")
        );
    }
    let audio = &value["output"]["audio"];
    let bytes = if let Some(data) = audio["data"].as_str().filter(|s| !s.is_empty()) {
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .context("百炼音频 Base64 无效")?
    } else {
        let download_url = audio["url"]
            .as_str()
            .filter(|s| !s.is_empty())
            .context("百炼响应缺少 output.audio.url / data")?;
        // Signed audio URL authorizes this download. Never forward the API bearer key.
        let parsed = reqwest::Url::parse(download_url).context("百炼音频 URL 无效")?;
        if !matches!(parsed.scheme(), "https" | "http") {
            bail!("百炼音频 URL 必须使用 HTTP 或 HTTPS");
        }
        client
            .get(parsed)
            .timeout(super::WHOLE_REQUEST_TIMEOUT)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .to_vec()
    };
    if bytes.is_empty() {
        bail!("百炼返回了空音频");
    }
    log::info!("[TTS] 百炼收到音频, 大小={}bytes", bytes.len());
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_endpoint_resolution() {
        for base in [
            "https://workspace.cn-beijing.maas.aliyuncs.com",
            "https://workspace.cn-beijing.maas.aliyuncs.com/api/v1/",
            "https://workspace.cn-beijing.maas.aliyuncs.com/v1",
        ] {
            assert_eq!(
                endpoint(base).unwrap(),
                format!("https://workspace.cn-beijing.maas.aliyuncs.com{ENDPOINT}")
            );
        }
        let full = format!("https://dashscope.aliyuncs.com{ENDPOINT}");
        assert_eq!(endpoint(&full).unwrap(), full);
        assert_eq!(endpoint(&(full.clone() + "#")).unwrap(), full);
        assert!(endpoint("https://example.com/v1/chat/completions").is_err());
        assert!(!matches(
            "https://api.siliconflow.cn",
            "FunAudioLLM/CosyVoice2-0.5B"
        ));
        assert!(!matches(
            "https://api.xiaomimimo.com/v1/chat/completions#",
            "mimo-v2.5-tts"
        ));
    }

    #[test]
    fn japanese_parameters_and_protocol_isolation() {
        let body = request_body("qwen-audio-3.1-tts-flash", "こんにちは", r#"{"voice":"longanhuan_v3.1","language_hints":["ja"],"speed":2,"response_format":"mp3","stream":true}"#).unwrap();
        assert_eq!(body["input"]["language_hints"], json!(["ja"]));
        assert_eq!(body["input"]["text"], "こんにちは");
        assert_eq!(body["input"]["format"], "wav");
        assert!(body["input"].get("speed").is_none());
        assert!(body.get("language_hints").is_none());
        let nested = request_body(
            "m",
            "original",
            r#"{"input":{"text":"override","language_hints":["ja"]}}"#,
        )
        .unwrap();
        assert_eq!(nested["input"]["text"], "original");
        assert_eq!(nested["input"]["language_hints"], json!(["ja"]));
    }

    #[test]
    fn invalid_parameters_fail_before_network() {
        for extra in [
            "[]",
            "invalid",
            r#"{"language_hints":"ja"}"#,
            r#"{"language_hints":[null]}"#,
            r#"{"voice":"model:alex"}"#,
            r#"{"format":"pcm"}"#,
        ] {
            assert!(request_body("m", "hi", extra).is_err(), "{extra}");
        }
    }

    // Real local HTTP exchange verifies auth, native body, signed URL download,
    // cache-compatible WAV encoding, and error responses without paid API calls.
    async fn server(
        response: Value,
        audio: Option<Vec<u8>>,
    ) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mut response = response;
        if audio.is_some() {
            response["output"]["audio"]["url"] = json!(format!("{base}/audio.wav"));
        }
        let handle = tokio::spawn(async move {
            for step in 0..if audio.is_some() { 2 } else { 1 } {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut req = Vec::new();
                let (head_end, length) = loop {
                    let mut buf = [0u8; 4096];
                    let n = socket.read(&mut buf).await.unwrap();
                    assert!(n > 0);
                    req.extend_from_slice(&buf[..n]);
                    if let Some(end) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&req[..end]).to_lowercase();
                        let length = headers
                            .lines()
                            .find_map(|s| s.strip_prefix("content-length:"))
                            .map(|s| s.trim().parse::<usize>().unwrap())
                            .unwrap_or(0);
                        if req.len() >= end + 4 + length {
                            break (end, length);
                        }
                    }
                };
                let headers = String::from_utf8_lossy(&req[..head_end]).to_lowercase();
                let bytes = if step == 0 {
                    assert!(headers.starts_with(&format!("post {} ", ENDPOINT.to_lowercase())));
                    assert!(headers.contains("authorization: bearer test-key"));
                    let body: Value =
                        serde_json::from_slice(&req[head_end + 4..head_end + 4 + length]).unwrap();
                    assert_eq!(body["input"]["language_hints"], json!(["ja"]));
                    assert_eq!(body["input"]["voice"], "longanhuan_v3.1");
                    serde_json::to_vec(&response).unwrap()
                } else {
                    assert!(headers.starts_with("get /audio.wav "));
                    assert!(!headers.contains("authorization:"));
                    audio.clone().unwrap()
                };
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                            bytes.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                socket.write_all(&bytes).await.unwrap();
            }
        });
        (base, handle)
    }

    #[tokio::test]
    async fn native_http_download_and_errors() {
        let wav = crate::tts::wrap_pcm16_wav(&[0u8; 48], 24000, 1);
        let (base, task) = server(
            json!({"output":{"audio":{}},"finish_reason":"stop"}),
            Some(wav.clone()),
        )
        .await;
        let client = Client::builder().no_proxy().build().unwrap();
        let result = synthesize(
            &client,
            &base,
            "test-key",
            "qwen-audio-3.1-tts-flash",
            r#"{"language_hints":["ja"]}"#,
            "こんにちは",
        )
        .await
        .unwrap();
        task.await.unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(result)
                .unwrap(),
            wav
        );
        let (base, task) = server(
            json!({"code":"InvalidParameter","message":"bad voice"}),
            None,
        )
        .await;
        let err = synthesize(
            &client,
            &base,
            "test-key",
            "qwen-audio-3.1-tts-flash",
            r#"{"language_hints":["ja"]}"#,
            "hi",
        )
        .await
        .unwrap_err();
        task.await.unwrap();
        assert!(err.to_string().contains("InvalidParameter"));
    }
}

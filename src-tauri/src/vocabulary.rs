//! UTF-8 vocabulary notebook. Limits count Unicode characters, not bytes.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const FILE_LIMIT: usize = 2000;

#[derive(serde::Deserialize)]
pub struct Entry {
    pub text: String,
    pub context: String,
    pub counterpart: String,
    pub side: String,
    pub saved_at: String,
}

#[derive(serde::Serialize)]
pub struct SaveResult {
    pub path: String,
    pub duplicate: bool,
}

fn excerpt(text: &str, limit: usize) -> String {
    let text = text.trim().replace("\r\n", "\n").replace('\r', "\n");
    if text.chars().count() <= limit {
        text
    } else {
        text.chars().take(limit - 1).chain(['…']).collect()
    }
}

fn render(entry: &Entry) -> anyhow::Result<(String, String)> {
    let text = entry.text.trim().replace("\r\n", "\n").replace('\r', "\n");
    if text.is_empty() {
        anyhow::bail!("请先在原文或译文中选中文字");
    }
    if text.chars().count() > 1000 {
        anyhow::bail!("单次收藏最多 1000 字，请缩小选区");
    }
    let context = excerpt(&entry.context, 500);
    let identity = format!("词语：{text}\n上下文：{context}\n");
    let side = if entry.side == "target" {
        "译文"
    } else {
        "原文"
    };
    let counterpart_label = if entry.side == "target" {
        "对应原文"
    } else {
        "对应译文"
    };
    let record = format!(
        "{identity}{counterpart_label}（节选）：{}\n选取位置：{side}\n收藏时间：{}\n--------------------\n\n",
        excerpt(&entry.counterpart, 300), excerpt(&entry.saved_at, 40)
    );
    anyhow::ensure!(record.chars().count() <= FILE_LIMIT, "收藏内容超过文件限制");
    Ok((record, identity))
}

fn numbered_files(directory: &Path) -> anyhow::Result<Vec<(u64, PathBuf)>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(number) = name
            .strip_prefix("生词本_")
            .and_then(|v| v.strip_suffix(".txt"))
        else {
            continue;
        };
        if !number.is_empty()
            && number.chars().all(|c| c.is_ascii_digit())
            && entry.file_type()?.is_file()
        {
            if let Ok(number) = number.parse::<u64>() {
                files.push((number, entry.path()));
            }
        }
    }
    files.sort_by_key(|(number, _)| *number);
    Ok(files)
}

pub fn save(directory: &Path, entry: &Entry) -> anyhow::Result<SaveResult> {
    let (record, identity) = render(entry)?;
    fs::create_dir_all(directory)?;
    // OS lock also serializes writers from separate application processes.
    // Closing the handle releases it even after an error or process crash.
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(".vocabulary.lock"))?;
    lock.lock()?;
    let files = numbered_files(directory)?;
    let mut last_length = 0;
    for (_, path) in &files {
        let content = fs::read_to_string(path)?;
        if content.contains(&identity) {
            return Ok(SaveResult {
                path: path.to_string_lossy().into_owned(),
                duplicate: true,
            });
        }
        last_length = content.chars().count();
    }
    let path = match files.last() {
        Some((_, path)) if last_length + record.chars().count() <= FILE_LIMIT => path.clone(),
        last => {
            let number = last
                .map_or(Some(1), |(number, _)| number.checked_add(1))
                .ok_or_else(|| anyhow::anyhow!("生词本文件编号已满"))?;
            directory.join(format!("生词本_{number:04}.txt"))
        }
    };
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(record.as_bytes())?;
    file.sync_data()?;
    Ok(SaveResult {
        path: path.to_string_lossy().into_owned(),
        duplicate: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            Self(std::env::temp_dir().join(format!(
                "dh-vocabulary-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn entry(text: &str) -> Entry {
        Entry {
            text: text.into(),
            context: "今日は日本語を勉強します。".into(),
            counterpart: "今天学习日语。".into(),
            side: "source".into(),
            saved_at: "2026-11-12 14:30:00".into(),
        }
    }

    #[test]
    fn unicode_rotation_restarts_and_preserves_complete_records() {
        let directory = Directory::new();
        let mut words = Vec::new();
        for i in 0..12 {
            let word = format!("{i}{}", "語😀".repeat(100));
            let result = save(&directory.0, &entry(&word)).unwrap();
            assert!(!result.duplicate);
            words.push(word);
        }
        let files = numbered_files(&directory.0).unwrap();
        assert!(files.len() > 1);
        let contents: Vec<_> = files
            .iter()
            .map(|(_, path)| fs::read_to_string(path).unwrap())
            .collect();
        for content in &contents {
            assert!(content.chars().count() <= FILE_LIMIT);
            assert!(content.ends_with("--------------------\n\n"));
        }
        for word in words {
            assert_eq!(
                contents
                    .iter()
                    .filter(|s| s.contains(&format!("词语：{word}\n")))
                    .count(),
                1
            );
        }
        // A new call rescans existing files, including after an app restart.
        assert!(
            save(&directory.0, &entry(&format!("0{}", "語😀".repeat(100))))
                .unwrap()
                .duplicate
        );
    }

    #[test]
    fn exact_limit_and_duplicate_do_not_create_extra_files() {
        let directory = Directory::new();
        fs::create_dir_all(&directory.0).unwrap();
        let item = entry("気になる");
        let length = render(&item).unwrap().0.chars().count();
        let first = directory.0.join("生词本_0001.txt");
        fs::write(&first, "字".repeat(FILE_LIMIT - length)).unwrap();
        assert_eq!(
            save(&directory.0, &item).unwrap().path,
            first.to_string_lossy()
        );
        assert_eq!(
            fs::read_to_string(&first).unwrap().chars().count(),
            FILE_LIMIT
        );
        assert!(save(&directory.0, &item).unwrap().duplicate);
        assert_eq!(numbered_files(&directory.0).unwrap().len(), 1);
        assert!(save(&directory.0, &entry("次の単語"))
            .unwrap()
            .path
            .ends_with("生词本_0002.txt"));
        assert_eq!(
            fs::read_to_string(&first).unwrap().chars().count(),
            FILE_LIMIT
        );
    }

    #[test]
    fn long_context_is_bounded_and_invalid_selection_writes_nothing() {
        let directory = Directory::new();
        assert!(save(&directory.0, &entry(" ")).is_err());
        assert!(!directory.0.exists());
        assert!(save(&directory.0, &entry(&"字".repeat(1001))).is_err());
        let mut item = entry(&"字".repeat(1000));
        item.context = "文".repeat(5000);
        item.counterpart = "訳".repeat(5000);
        let result = save(&directory.0, &item).unwrap();
        let content = fs::read_to_string(result.path).unwrap();
        assert!(content.chars().count() <= FILE_LIMIT);
        assert!(content.contains(&format!("词语：{}\n", item.text)));
        assert!(content.contains('…'));
    }

    #[test]
    fn concurrent_writers_keep_all_records_and_file_limit() {
        let directory = Directory::new();
        let handles: Vec<_> = (0..12)
            .map(|i| {
                let path = directory.0.clone();
                std::thread::spawn(move || save(&path, &entry(&format!("単語{i}"))).unwrap())
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let contents: Vec<_> = numbered_files(&directory.0)
            .unwrap()
            .iter()
            .map(|(_, p)| fs::read_to_string(p).unwrap())
            .collect();
        assert!(contents.iter().all(|s| s.chars().count() <= FILE_LIMIT));
        for i in 0..12 {
            assert_eq!(
                contents
                    .iter()
                    .map(|s| s.matches(&format!("词语：単語{i}\n")).count())
                    .sum::<usize>(),
                1
            );
        }
    }
}

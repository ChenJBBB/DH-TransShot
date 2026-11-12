import { LanguageSelector } from "./LanguageSelector";
import { SwapButton } from "./SwapButton";
import { TextArea } from "./TextArea";
import { ActionButtons } from "./ActionButtons";
import { useTranslation } from "../../hooks/useTranslation";
import { useTranslationStore } from "../../stores/translationStore";
import { useRef, useState } from "react";
import { saveVocabulary } from "../../lib/invoke";
import { selectionContext } from "../../lib/vocabulary";
import { appLog } from "../../stores/logStore";

export function TranslationPanel() {
  const savingVocabulary = useRef(false);
  const [vocabularyNotice, setVocabularyNotice] = useState<string | null>(null);
  const {
    sourceText,
    translatedText,
    isTranslating,
    isOcrProcessing,
    error,
    setSourceText,
    translate,
  } = useTranslation();

  const { sourceLang, targetLang, setSourceLang, setTargetLang, swapLanguages } =
    useTranslationStore();

  const collectSelection = async () => {
    if (savingVocabulary.current) return;
    const textarea = document.activeElement;
    if (!(textarea instanceof HTMLTextAreaElement) || !textarea.dataset.vocabularySide || textarea.selectionStart === textarea.selectionEnd) {
      setVocabularyNotice("请先在原文或译文中选中文字，再按 Alt+C");
      return;
    }
    if (isOcrProcessing || isTranslating) {
      setVocabularyNotice("请等待识别和翻译完成后再收藏");
      return;
    }
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    const text = textarea.value.slice(start, end).trim();
    if (!text) {
      setVocabularyNotice("请先选中词语");
      return;
    }
    const side = textarea.dataset.vocabularySide === "target" ? "target" : "source";
    savingVocabulary.current = true;
    setVocabularyNotice("正在收藏…");
    try {
      const result = await saveVocabulary({
        text,
        context: selectionContext(textarea.value, start, end),
        counterpart: side === "source" ? translatedText : sourceText,
        side,
        saved_at: new Date().toLocaleString(),
      });
      const name = result.path.split(/[\\/]/).pop();
      setVocabularyNotice(result.duplicate ? `已收藏过 · ${name}` : `已收藏到 ${name}`);
      appLog.info("[Vocabulary] 收藏完成, 重复=" + result.duplicate + ", 文件=" + name);
    } catch (e) {
      setVocabularyNotice(String(e));
      appLog.error("[Vocabulary] 收藏失败: " + String(e));
    } finally {
      savingVocabulary.current = false;
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey && e.code === "KeyC") {
      e.preventDefault();
      e.stopPropagation();
      if (!e.repeat && !e.nativeEvent.isComposing) void collectSelection();
      return;
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      translate();
    }
  };

  return (
    <div className="flex flex-col flex-1 px-4 pb-4 pt-1 gap-3 overflow-hidden" onKeyDown={handleKeyDown}>
      {/* Source text card */}
      <div
        className="flex-1 flex flex-col min-h-0 rounded-xl overflow-hidden"
        style={{ backgroundColor: "var(--color-surface)" }}
      >
        <TextArea
          vocabularySide="source"
          value={isOcrProcessing ? "识别中..." : sourceText}
          onChange={setSourceText}
          placeholder="输入要翻译的文本... (Ctrl+Enter)"
          readOnly={isOcrProcessing}
        />
        <ActionButtons text={sourceText} speakId="source" />
      </div>

      {/* Language selection bar */}
      <div className="flex items-center justify-center px-1 py-0.5">
        <div className="flex items-center gap-3">
          <LanguageSelector value={sourceLang} onChange={setSourceLang} includeAuto />
          <SwapButton onClick={swapLanguages} disabled={sourceLang === "auto"} />
          <LanguageSelector value={targetLang} onChange={setTargetLang} />
        </div>
      </div>

      {/* Translation result card */}
      <div
        className="flex-1 flex flex-col min-h-0 rounded-xl overflow-hidden"
        style={{ backgroundColor: "var(--color-surface)" }}
      >
        <TextArea
          vocabularySide="target"
          value={isOcrProcessing ? "" : isTranslating ? "翻译中..." : translatedText}
          readOnly
          placeholder="翻译结果将显示在这里..."
        />
        <ActionButtons text={translatedText} speakId="target" />
      </div>

      <div className="shrink-0 flex items-center gap-2 text-xs" style={{ color: "var(--color-text-secondary)" }}>
        <button
          type="button"
          className="shrink-0 rounded-md px-2 py-1 hover:bg-black/5"
          title="在原文或译文中选中文字，按 Alt+C 收藏"
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => void collectSelection()}
        >
          ☆ 收藏选词 · Alt+C
        </button>
        <span role="status" className="truncate" title={vocabularyNotice ?? ""}>
          {vocabularyNotice ?? "自动保存上下文 · 每个 TXT 最多 2000 字"}
        </span>
      </div>

      {/* Error */}
      {error && (
        <div className="text-xs text-red-500 px-1">{error}</div>
      )}
    </div>
  );
}

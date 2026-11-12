import { useEffect, useState } from "react";
import { getVocabularyDirectory, openVocabularyDirectory } from "../../lib/invoke";
import { appLog } from "../../stores/logStore";
import { Field, InfoNote, SectionHeader, TextInput } from "./controls";

export function VocabularySettings() {
  const [path, setPath] = useState("");
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    getVocabularyDirectory().then(setPath).catch((e) => setError(String(e)));
  }, []);

  const openDirectory = async () => {
    setError(null);
    try {
      await openVocabularyDirectory();
      appLog.info("[Vocabulary] 打开生词本文件夹");
    } catch (e) {
      setError(String(e));
      appLog.error("[Vocabulary] 打开文件夹失败: " + String(e));
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <SectionHeader title="生词本" description="在翻译界面选中词语，按 Alt+C 即可收藏。" />
      <Field label="文件存储位置">
        <TextInput value={path} readOnly title={path} aria-label="生词本存储位置" />
      </Field>
      <button
        type="button"
        className="self-start rounded-lg px-4 py-2 text-sm hover:opacity-80"
        style={{ backgroundColor: "var(--color-primary)", color: "white" }}
        onClick={() => void openDirectory()}
      >
        打开存储文件夹
      </button>
      <InfoNote>
        文件保存在系统“文档”文件夹下的 DH-TransShot / 生词本中，退出软件后仍会保留。
        每个 UTF-8 TXT 最多 2000 个字符（含标点、空格和换行），放不下整条收藏时自动新建下一个编号文件。
        收藏包含词语、所在句子、对应原文或译文节选和收藏时间；过长上下文会截取。
        单次选词最多 1000 字，同词同上下文会自动去重。
      </InfoNote>
      {error && <p role="alert" className="text-xs text-red-500">{error}</p>}
    </div>
  );
}

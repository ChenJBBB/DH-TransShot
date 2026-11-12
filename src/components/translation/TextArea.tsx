interface Props {
  value: string;
  onChange?: (value: string) => void;
  placeholder?: string;
  readOnly?: boolean;
  vocabularySide?: "source" | "target";
}

export function TextArea({ value, onChange, placeholder, readOnly = false, vocabularySide }: Props) {
  return (
    <textarea
      data-vocabulary-side={vocabularySide}
      aria-label={vocabularySide === "source" ? "原文" : vocabularySide === "target" ? "译文" : undefined}
      value={value}
      onChange={onChange ? (e) => onChange(e.target.value) : undefined}
      placeholder={placeholder}
      readOnly={readOnly}
      className="w-full flex-1 resize-none text-sm leading-relaxed outline-none"
      style={{
        backgroundColor: "transparent",
        color: "var(--color-text)",
        padding: "8px 12px",
      }}
    />
  );
}

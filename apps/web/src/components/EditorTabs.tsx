const tabs = ["Sources", "Story", "Edit", "Agent", "Review", "Deliverables"] as const

export type EditorTab = (typeof tabs)[number]

export function EditorTabs({
  active,
  onSelect,
}: {
  readonly active: EditorTab
  readonly onSelect: (tab: EditorTab) => void
}) {
  return (
    <nav className="tabs" aria-label="Workspace">
      {tabs.map((tab) => (
        <button
          type="button"
          key={tab}
          className={active === tab ? "active" : ""}
          aria-current={active === tab ? "page" : undefined}
          onClick={() => onSelect(tab)}
        >
          {tab}
        </button>
      ))}
    </nav>
  )
}

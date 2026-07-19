import type { ReactNode } from 'react';

interface SidebarSection {
  key: string;
  title: string;
  body: ReactNode;
}

interface Props {
  sections: SidebarSection[];
  collapsed?: boolean;
}

/** Right sidebar (spec §4.3 "会话/Agent/Plan" rail). Sections stack vertically;
 *  each has a title + body. Subproject 4 fills in SessionPicker (always) +
 *  AgentPanel (Task 4.6); Plan section is reserved for subproject 5+. */
export function Sidebar({ sections, collapsed = false }: Props) {
  if (collapsed) return null;
  return (
    <aside className="app-sidebar" data-testid="sidebar">
      {sections.map((s) => (
        <section key={s.key} className={`sidebar-section sidebar-${s.key}`}>
          <h3 className="sidebar-section-title">{s.title}</h3>
          <div className="sidebar-section-body">{s.body}</div>
        </section>
      ))}
    </aside>
  );
}

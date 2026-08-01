import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'userMessage' }>;
}

/** User-authored message cell. Renders the plain text; subproject 3 does not
 *  handle image/audio/skill/mention inputs (those land in the composer). */
export function UserMessageCell({ model }: Props) {
  return (
    <div className="cell cell-user" data-testid="user-message">
      <div className="cell-role">user</div>
      <div className="cell-body">
        <pre className="cell-text">{model.text}</pre>
      </div>
    </div>
  );
}

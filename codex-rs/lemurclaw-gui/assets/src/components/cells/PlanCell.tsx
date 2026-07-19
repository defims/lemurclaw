import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'plan' }>;
}

/** Plan cell. Renders the plan text verbatim (subproject 3 does not implement
 *  live plan editing — thread/goal/set is deferred). */
export function PlanCell({ model }: Props) {
  return (
    <div className="cell cell-plan" data-testid="plan">
      <div className="cell-plan-header">📋 plan</div>
      <pre className="cell-plan-text">{model.text || '(empty plan)'}</pre>
    </div>
  );
}

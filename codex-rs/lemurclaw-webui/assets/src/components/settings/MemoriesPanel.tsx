import { SettingsForm } from './SettingsForm';

/** Memories editor. Backed by `developer_instructions` (a typed Option<String>
 *  field on the wire Config). This is the free-form text the agent sees as
 *  persistent memory across turns. */
export function MemoriesPanel() {
  return (
    <SettingsForm
      configKey="developerInstructions"
      writeKeyPath="developer_instructions"
      label="Memories"
      hint="Free-form text shown to the agent as persistent memory."
    />
  );
}

import { SettingsForm } from './SettingsForm';

/** Model settings editor. Exposes the model-related scalar fields on Config
 *  that the app-server actually serializes: model id, provider, reasoning
 *  effort, and verbosity. Each is its own <SettingsForm> row. */
export function ModelPanel() {
  return (
    <div className="settings-form-stack">
      <SettingsForm configKey="model" label="Model id" hint="e.g. gpt-5.2" />
      <SettingsForm configKey="modelProvider" writeKeyPath="model_provider" label="Model provider" hint="e.g. openai, openai-compatible" />
      <SettingsForm configKey="modelReasoningEffort" writeKeyPath="model_reasoning_effort" label="Reasoning effort" hint="minimal | low | medium | high" />
      <SettingsForm configKey="modelVerbosity" writeKeyPath="model_verbosity" label="Verbosity" hint="default | verbose" />
    </div>
  );
}

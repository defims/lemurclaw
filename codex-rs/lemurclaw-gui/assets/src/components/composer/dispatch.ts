import type { SlashCommand, SlashCommandContext, SlashCommandResult } from './slashCommandTypes';

/** Dispatch a slash command.
 *
 *  Thin pass-through today — each command carries its own dispatch fn in the
 *  catalog (see slashCommands.ts). Having a named entry point leaves room for
 *  cross-cutting concerns (logging, metrics, error handling) later without
 *  changing call sites. */
export function dispatchSlashCommand(
  cmd: SlashCommand,
  args: string,
  ctx: SlashCommandContext,
): SlashCommandResult {
  return cmd.dispatch(args, ctx);
}

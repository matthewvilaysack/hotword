// OpenCode plugin for hotword. Copy to ~/.config/opencode/plugins/hotword.ts
// (or a repo's .opencode/plugins/). Needs the hotword binary on PATH.
//
// Prompt phrases: when a user message contains a trigger, the workflow report
// is appended to that message inside <hotword> tags.
// Session start: workflows with on = ["session-start"] run once per session and
// their report rides along in the system prompt for that session.
import type { Plugin } from "@opencode-ai/plugin"

export const HotwordPlugin: Plugin = async ({ $, directory }) => {
  const sessionContext = new Map<string, string | undefined>()

  async function hook(event: "prompt" | "session-start", payload: Record<string, string>) {
    const input = JSON.stringify({ ...payload, cwd: directory })
    const result = await $`echo ${input} | hotword hook ${event}`.quiet().nothrow()
    const text = result.stdout.toString().trim()
    if (!text) return undefined
    try {
      const parsed = JSON.parse(text) as { hookSpecificOutput?: { additionalContext?: string } }
      return parsed.hookSpecificOutput?.additionalContext
    } catch {
      return undefined
    }
  }

  return {
    "chat.message": async (_input, output) => {
      const part = output.parts.find((p) => p.type === "text")
      if (!part || part.type !== "text") return
      const context = await hook("prompt", { prompt: part.text })
      if (context) part.text += `\n\n<hotword>\n${context}\n</hotword>`
    },

    "experimental.chat.system.transform": async (input, output) => {
      const id = input.sessionID ?? "no-session"
      if (!sessionContext.has(id)) {
        sessionContext.set(id, await hook("session-start", { source: "startup" }))
      }
      const context = sessionContext.get(id)
      if (context) output.system.push(`<hotword>\n${context}\n</hotword>`)
    },
  }
}

export default HotwordPlugin

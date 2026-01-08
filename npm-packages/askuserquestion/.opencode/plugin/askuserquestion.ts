import type { Plugin } from "@opencode-ai/plugin"
import { tool } from "@opencode-ai/plugin/tool"
import { spawn } from "child_process"
import { createRequire } from "module"
import { join } from "path"
import { existsSync, writeFileSync, unlinkSync } from "fs"
import { tmpdir } from "os"
import { randomUUID } from "crypto"

const require = createRequire(import.meta.url)

// Platform-specific package names
const PLATFORM_PACKAGES: Record<string, string> = {
  "darwin-arm64": "@kirmad/askuserquestion-darwin-arm64",
  "darwin-x64": "@kirmad/askuserquestion-darwin-x64",
  "linux-x64": "@kirmad/askuserquestion-linux-x64",
  "linux-arm64": "@kirmad/askuserquestion-linux-arm64",
  "win32-x64": "@kirmad/askuserquestion-win32-x64",
}

const BINARY_NAMES: Record<string, string> = {
  darwin: "askuserquestion",
  linux: "askuserquestion",
  win32: "askuserquestion.exe",
}

function getBinaryPath(): string {
  const platformKey = `${process.platform}-${process.arch}`
  const packageName = PLATFORM_PACKAGES[platformKey]

  if (!packageName) {
    throw new Error(
      `Unsupported platform: ${platformKey}. Supported: ${Object.keys(PLATFORM_PACKAGES).join(", ")}`
    )
  }

  try {
    const packagePath = require.resolve(packageName)
    const packageDir = join(packagePath, "..")
    const binaryName = BINARY_NAMES[process.platform] || "askuserquestion"
    const binaryPath = join(packageDir, binaryName)

    if (!existsSync(binaryPath)) {
      throw new Error(`Binary not found at: ${binaryPath}`)
    }

    return binaryPath
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "MODULE_NOT_FOUND") {
      throw new Error(`Platform package not installed: ${packageName}`)
    }
    throw err
  }
}

interface QuestionOption {
  label: string
  description: string
}

interface Question {
  question: string
  header: string
  options: QuestionOption[]
  multiSelect: boolean
}

interface QuestionAnswer {
  question: string
  header: string
  selected?: string | string[]
  selected_index?: number | number[]
}

interface BinaryResponse {
  status: "selected" | "cancelled"
  answers: QuestionAnswer[]
}

async function playNotificationSound(): Promise<void> {
  return new Promise((resolve) => {
    let command: string
    let args: string[]

    if (process.platform === "darwin") {
      command = "afplay"
      args = ["/System/Library/Sounds/Glass.aiff"]
    } else if (process.platform === "win32") {
      command = "powershell"
      args = ["-c", "[System.Media.SystemSounds]::Exclamation.Play()"]
    } else {
      command = "paplay"
      args = ["/usr/share/sounds/freedesktop/stereo/bell.oga"]
    }

    const proc = spawn(command, args, { stdio: "ignore", detached: true })
    proc.on("error", () => {})
    proc.unref()
    resolve()
  })
}

async function askUserQuestions(questions: Question[]): Promise<{
  status: "selected" | "cancelled" | "error"
  answers: Record<string, string | string[]>
  raw?: QuestionAnswer[]
  error?: string
}> {
  const binaryPath = getBinaryPath()
  const tempFile = join(tmpdir(), `askuserquestion-${randomUUID()}.json`)

  try {
    writeFileSync(tempFile, JSON.stringify({ questions }))
    playNotificationSound()

    const result = await new Promise<string>((resolve, reject) => {
      const proc = spawn(binaryPath, ["--input", tempFile], {
        stdio: ["ignore", "pipe", "pipe"],
      })

      let stdout = ""
      let stderr = ""

      proc.stdout.on("data", (data) => (stdout += data.toString()))
      proc.stderr.on("data", (data) => (stderr += data.toString()))

      proc.on("close", (code) => {
        if (code !== 0 && stderr) {
          reject(new Error(`Binary exited with code ${code}: ${stderr}`))
        } else {
          resolve(stdout)
        }
      })

      proc.on("error", reject)
    })

    const trimmed = result.trim()
    if (!trimmed) {
      return { status: "cancelled", answers: {} }
    }

    const response: BinaryResponse = JSON.parse(trimmed)

    if (response.status === "cancelled") {
      return { status: "cancelled", answers: {} }
    }

    const answers: Record<string, string | string[]> = {}
    for (const answer of response.answers) {
      const key = answer.header || answer.question
      answers[key] = answer.selected ?? ""
    }

    return { status: "selected", answers, raw: response.answers }
  } catch (error) {
    return {
      status: "error",
      answers: {},
      error: error instanceof Error ? error.message : "Unknown error",
    }
  } finally {
    try {
      unlinkSync(tempFile)
    } catch {}
  }
}

/**
 * OpenCode plugin that registers the AskUserQuestion tool
 */
export const AskUserQuestionPlugin: Plugin = async (_ctx) => {
  return {
    tool: {
      AskUserQuestion: tool({
        description: `Use this tool when you need to ask the user questions during execution. This allows you to:
1. Gather user preferences or requirements
2. Clarify ambiguous instructions
3. Get decisions on implementation choices as you work
4. Offer choices to the user about what direction to take.

Usage notes:
- Users will always be able to select "Other" to provide custom text input
- Use multiSelect: true to allow multiple answers to be selected for a question
- If you recommend a specific option, make that the first option in the list and add "(Recommended)" at the end of the label`,
        args: {
          questions: tool.schema
            .array(
              tool.schema.object({
                question: tool.schema
                  .string()
                  .describe(
                    'The complete question to ask the user. Should be clear, specific, and end with a question mark. Example: "Which library should we use for date formatting?"'
                  ),
                header: tool.schema
                  .string()
                  .describe(
                    'Very short label displayed as a chip/tag (max 12 chars). Examples: "Auth method", "Library", "Approach".'
                  ),
                options: tool.schema
                  .array(
                    tool.schema.object({
                      label: tool.schema
                        .string()
                        .describe("The display text for this option (1-5 words)."),
                      description: tool.schema
                        .string()
                        .describe("Explanation of what this option means."),
                    })
                  )
                  .min(2)
                  .max(4)
                  .describe("The available choices (2-4 options). No 'Other' option needed - it's added automatically."),
                multiSelect: tool.schema
                  .boolean()
                  .describe("Set to true to allow multiple selections."),
              })
            )
            .min(1)
            .max(4)
            .describe("Questions to ask the user (1-4 questions)"),
        },
        async execute(args) {
          const result = await askUserQuestions(args.questions)
          return JSON.stringify(result)
        },
      }),
    },
  }
}

export default AskUserQuestionPlugin

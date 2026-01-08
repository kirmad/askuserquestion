# @kirmad/askuserquestion

OpenCode plugin that provides a native GUI dialog for asking users questions. Works cross-platform with prebuilt binaries for macOS, Linux, and Windows.

## Features

- Native GUI dialogs (not browser-based)
- Cross-platform support (macOS, Linux, Windows)
- Single-select and multi-select questions
- Custom "Other" option with free-text input
- Beautiful dark theme UI
- Sound notification when dialog appears

## Installation

```bash
npm install @kirmad/askuserquestion
```

The correct platform-specific binary is automatically installed based on your OS and architecture.

## Usage

Add to your `opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "plugin": ["@kirmad/askuserquestion"]
}
```

This registers the `AskUserQuestion` tool automatically.

## Tool: AskUserQuestion

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `questions` | `Question[]` | 1-4 questions to ask |

### Question Object

| Property | Type | Description |
|----------|------|-------------|
| `question` | `string` | The full question text |
| `header` | `string` | Short label (max 12 chars) |
| `options` | `QuestionOption[]` | 2-4 options to choose from |
| `multiSelect` | `boolean` | Allow multiple selections |

### QuestionOption Object

| Property | Type | Description |
|----------|------|-------------|
| `label` | `string` | Display text for the option |
| `description` | `string` | Explanation of what this option means |

### Response

```json
{
  "status": "selected",
  "answers": {
    "Auth": "OAuth 2.0",
    "Features": ["Dark mode", "Analytics"]
  }
}
```

Status can be `"selected"`, `"cancelled"`, or `"error"`.

## Supported Platforms

| Platform | Architecture | Package |
|----------|-------------|---------|
| macOS | Apple Silicon (M1/M2/M3) | `@kirmad/askuserquestion-darwin-arm64` |
| macOS | Intel | `@kirmad/askuserquestion-darwin-x64` |
| Linux | x64 | `@kirmad/askuserquestion-linux-x64` |
| Linux | ARM64 | `@kirmad/askuserquestion-linux-arm64` |
| Windows | x64 | `@kirmad/askuserquestion-win32-x64` |

## License

MIT

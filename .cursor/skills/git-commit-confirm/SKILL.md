---
name: git-commit-confirm
description: Provides a confirmation mechanism during git commit in Cursor. Displays a change summary and obtains explicit user confirmation before committing. Use when the user requests to commit changes with terms like "commit", "提交", "commit一下", etc. Skip confirmation only when user explicitly requests to force commit or skip confirmation.
---

# Git Commit Confirmation

This skill ensures all commit content is fully reviewed before committing by requiring the agent to display a change summary and obtain explicit user confirmation.

## Usage Scenario

- **Trigger**: When users request to commit changes (including "commit", "提交", "commit一下", etc.)
- **Skip Condition**: If users explicitly request skipping confirmation (such as "force commit", "skip confirmation", "直接提交", etc.), commit directly

## Workflow

### Step 1: Check Skip Conditions

Read the user's latest request. If any of the following exist, skip all confirmation steps and commit directly:
- Explicit skip request: "force commit", "skip confirmation", "no confirm", "直接提交", "不用确认"
- Explicit original method request: "use original commit", "不确认直接提交"

### Step 2: Display Changes and Request Confirmation

If not skipping, perform these operations:

1. **Check git status** for untracked files:
   ```bash
   git status
   ```

2. **View detailed changes**:
   ```bash
   git diff          # unstaged changes
   git diff --cached # staged changes
   ```

3. **Analyze and summarize changes**:
   - Review all changes (staged and unstaged)
   - Identify: new files, modifications, deletions, files involved

4. **Present summary to user**:
   - Display change summary
   - Mark if untracked files exist
   - Ask for explicit confirmation

### Step 3: Handle Confirmation

- **Confirmed** ("yes", "confirm", "提交", "确认"):
  - If unstaged changes exist: `git add .`
  - Execute commit with original logic
  
- **Declined or vague** ("no", "cancel", "wait", "我再看看"):
  - Do not commit
  - Inform user commit was cancelled

## Important Notes

- When skipping: commit directly without explaining the skip
- When confirming: only explicit agreement counts; vague answers are refusal
- When refusing: respect decision without redundant explanations
- Confirmation status does not persist across messages; re-confirm on new commit requests
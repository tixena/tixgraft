# GitHub Actions Workflows

This directory contains the CI/CD workflows for TixGraft.

## Workflows

### CI/CD Pipeline (`ci.yml`)

Runs on every push and pull request to `main`/`master` branches.

**Jobs:**
- **Test**: Runs formatting checks, linting, tests, and builds release binary
- **Security Audit**: Runs `cargo audit` to check for security vulnerabilities
- **Notify**: Sends build status to Discord (if configured)

## Discord Notifications Setup

The CI workflow can send notifications to Discord about build successes and failures.

### 1. Create a Discord Webhook

1. Go to your Discord server
2. Navigate to **Server Settings** → **Integrations** → **Webhooks**
3. Click **New Webhook** or **Create Webhook**
4. Give it a name (e.g., "TixGraft CI")
5. Select the channel where notifications should be posted
6. Click **Copy Webhook URL**
7. Click **Save**

### 2. Add the Webhook to GitHub Secrets

1. Go to your GitHub repository
2. Navigate to **Settings** → **Secrets and variables** → **Actions**
3. Click **New repository secret**
4. Name: `DISCORD_WEBHOOK_URL`
5. Value: Paste the Discord webhook URL you copied
6. Click **Add secret**

### 3. Test the Integration

1. Push a commit or create a pull request
2. Check the Actions tab to see the workflow running
3. Verify that a notification appears in your Discord channel

## Security Notes

- The Discord webhook URL is stored as a GitHub secret and is never exposed in logs
- The workflow checks if the secret exists before attempting to send notifications
- If the secret is not configured, the workflow will still run successfully but skip Discord notifications

## Notification Format

**Success notifications include:**
- Repository name
- Branch and commit information
- Author name
- Commit message
- Status of each job (tests, security audit)

**Failure notifications include:**
- All success information plus:
- Urgent markers and @here mention
- Link to the failed workflow run
- Action required message

## Customization

You can customize the Discord notifications by editing the `notify` job in [ci.yml](./ci.yml):

- Change the message format
- Add/remove fields
- Modify colors (use decimal RGB values)
- Adjust when notifications are sent (success only, failures only, always)

## Local Testing

To test the workflow locally before pushing:

```bash
# Run the same checks that CI runs
just ci

# Or run individual steps
just fmt-check
just lint
just test
just build-release
```

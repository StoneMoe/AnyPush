![](img/logo.png)

# Feature
1. Accepting incoming email as SMTP Server, and send it with HTTP (or Webhook).
2. For Synology NAS email notification redirect currently. 
3. More incoming & output options in the future.

# Usage
Use docker:
```bash
docker run --name=anypush --restart=unless-stopped -e WEBHOOK_URL=https://example.com/api -p 0.0.0.0:1025:1025 stonemoe/anypush:1.0.0
```
Then AnyPush will:
```
POST https://example.com/api with JSON:
{
  'from': "sender@example.com",
  'to': "to@example.com",
  'subject': "mail subject",
  'text': "mail content here"
}
```

# Notice
This project is still under heavily development.

You may encounter issues including:
1. Breaking API compatibility between commits
2. Trash hard-code
3. Crash in some case
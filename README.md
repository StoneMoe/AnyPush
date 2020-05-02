![](img/logo.png)

# Feature
1. Accepting incoming email as SMTP Server, and send it with HTTP (or Webhook).
2. For Synology NAS email notification redirect currently. 
3. More incoming & output options in the future.

# Installation
### Start Docker container
```bash
docker run --name=anypush --restart=unless-stopped -e WEBHOOK_URL=https://example.com/api -p 0.0.0.0:587:587 stonemoe/anypush:tag_name
```
Check stable release tags in [Release page](https://github.com/StoneMoe/AnyPush/releases)  
Check all available tags in [DockerHub repository](https://hub.docker.com/r/stonemoe/anypush/tags)

### Test it
```bash
$ telnet localhost 587
Trying localhost...
Connected to localhost.
Escape character is '^]'.
220 anypush Python SMTP proxy version 0.3
HELO localhost
250 anypush
mail from: sender@example.com
250 OK
rcpt to: to@example.com
250 OK
data
354 End data with <CR><LF>.<CR><LF>
Hey
This is test email

250 OK
QUIT
221 Bye
```

Then AnyPush will:
```
POST https://example.com/api with JSON:
{
  "from": "sender@example.com",
  "to": ["to@example.com"],
  "subject": "mail subject",
  "text": "mail content here"
}
```

# Notice
This project is still under heavily development.

You may encounter issues including:
1. Breaking API compatibility between commits
2. Trash hard-code
3. Crash in some case
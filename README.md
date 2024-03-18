Configuration Example

File should be at `~/.my-reverse-proxy` location with yaml format:

```yaml
hosts:
  localhost:8000:
  - location: /
    proxy_pass_to: ssh:username@ssh_host:22->remote_host:5123
  localhost:8001:
  - location: /
    proxy_pass_to: http://remote_host:5123
```


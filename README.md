# Configuration Example

File should be at `~/.my-reverse-proxy` location with yaml format:

```yaml
hosts:
  localhost:8000:
  - type: http1
    location: /    
    proxy_pass_to: ssh:username@ssh_host:22->remote_host:5123
  localhost:8001:
  - type: http1
    location: /    
    proxy_pass_to: http://remote_host:5123
```




## On Development 

* Ability to setup tcp proxy

```yaml
hosts:
  localhost:8000:
  - type: tcp
    proxy_pass_to: remote_host:5123
```

* Ability to setup Http2 proxy

```yaml
hosts:
  localhost:8000:
  - type: http2
    location: /    
    proxy_pass_to: ssh:username@ssh_host:22->remote_host:5123
  localhost:8001:
  - type: http2
    location: /    
    proxy_pass_to: http://remote_host:5123    
```
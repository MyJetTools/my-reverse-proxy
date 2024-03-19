# Configuration Example

File should be at `~/.my-reverse-proxy` location with yaml format:

```yaml
connection_settings:
  buffer_size: 512Kb # Buffer, which is allocated twice (read/write) per connection to pass traffic by
  connect_to_remote_timeout: 5s # Timeout to connect to remote host
  
hosts:
  localhost:8000:
  - type: http1
    location: /    
    proxy_pass_to: ssh:username@ssh_host:22->remote_host:5123

  localhost:8001:
  - type: http1
    location: /    
    proxy_pass_to: http://remote_host:5123

  localhost:8002:
  - type: tcp
    proxy_pass_to: 10.0.0.5:5123    

  localhost:8003:
  - type: tcp
    proxy_pass_to: ssh:username@ssh_host:22->10.0.0.5:5123    

  localhost:8005:
  - type: http2
    location: /service1    
    proxy_pass_to: ${my_ssh_config}->remote_host:5123

  localhost:8006:
  - type: http2
    location: /service2
    proxy_pass_to: http://remote_host:5123  
    
variables:
  my_ssh_config: ssh:user@10.12.13.14:22
```

```yaml
 
```
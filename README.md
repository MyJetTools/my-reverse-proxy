# Configuration Example

File should be at `~/.my-reverse-proxy` location with yaml format:

```yaml
connection_settings:
  buffer_size: 512Kb # Buffer, which is allocated twice (read/write) per connection to pass traffic by
  connect_to_remote_timeout: 5s # Timeout to connect to remote host
  
hosts:
  localhost:8000:
    endpoint:
      type: https
      ssl_certificate: my_ssl_cert  
    locations:
    - type: http
      proxy_pass_to: ssh:username@ssh_host:22->remote_host:5123      

  localhost:8001:
    endpoint:
      type: http
    locations:      
    - type: http
      proxy_pass_to: http://remote_host:5123

  localhost:8002:
    endpoint:
      type: tcp  
    locations:        
    - proxy_pass_to: 10.0.0.5:5123    

  localhost:8003:
    endpoint:
      type: tcp
    locations:      
    - proxy_pass_to: ssh:username@ssh_host:22->10.0.0.5:5123    

  8005:
    endpoint:
      type: http2  

    locations:       
    - type: http2
      path: /service1    
      proxy_pass_to: ${my_ssh_config}->remote_host:5123
    - type: http2
      path: /service2
      proxy_pass_to: http://remote_host:5123  

ssl_certificates:
  - id: my_ssl_cert
    certificate: ~/certs/cert.cer
    private_key: ~/certs/cert.key    
    
variables:
  my_ssh_config: ssh:user@10.12.13.14:22
```

```yaml
 
```
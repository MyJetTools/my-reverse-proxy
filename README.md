# Configuration Example

File should be at `~/.my-reverse-proxy` location with yaml format:

```yaml
global_settings:
  connection_settings:
    buffer_size: 512Kb # Buffer, which is allocated twice (read/write) per connection to pass traffic by
    connect_to_remote_timeout: 5s # Timeout to connect to remote host
  
hosts:
  localhost:8000:
    endpoint:
      type: https
      ssl_certificate: my_ssl_cert  
      client_certificate_ca: ca_id

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
    - path: /service1
      type: http2          
      proxy_pass_to: ${my_ssh_config}->remote_host:5123
    - path: /service2
      type: http2     
      proxy_pass_to: http://remote_host:5123  

ssl_certificates:
  - id: my_ssl_cert
    certificate: ~/certs/cert.cer
    private_key: ~/certs/cert.key  

client_certificate_ca:
  - id: ca_id
    ca: ~/certs/ca.cer  
    
variables:
  my_ssh_config: ssh:user@10.12.13.14:22
```

## Http request endpoints
### Headers
By default all the headers of each request are passed to headers of each response accordingly both ways (ServerRequest->RemoteRequest and RemoteResponse->ServerResponse);

It is possible to add custom headers to request by adding yaml section:

Globally - add or remove headers to each request on each endpoint
```yaml
global_settings:
  all_http_endpoints:
    modify_http_headers:
      add:
        request:
        - name: x-real-ip
          value: '${ENDPOINT_IP}'
        response:
        - name: header-name1: 
          value: value1
        - name: header-name2: 
          value: value2
      remove:
        request:
        - header-name1
        - header-name2
        response:
        - header-name3
        - header-name4

```

On endpoint level - add header to each endpoint
```yaml
hosts:
  localhost:8000:
    endpoint:
      type: http  
      modify_http_headers:      
        add:
          request:
          - name: x-real-ip
            value: '${ENDPOINT_IP}'
          response:
          - name: header-name1
            value: value1
          - name: header-name2
            value: value2
        remove:
          request:
          - header-name1
          - header-name2
          response:
          - header-name3
          - header-name4        
```

On location level - add header to each endpoint

```yaml
hosts:
  localhost:8001:
    endpoint:
      type: http
    locations:      
    - type: http
      proxy_pass_to: http://remote_host:5123
      modify_http_headers:         
        add:
          request:
          - name: x-real-ip
            value: '${ENDPOINT_IP}'
          response:
          - name: header-name1
            value: value1
          - name: header-name2
            value: value2:
        remove:
          request:
          - header-name1
          - header-name2
          response:
          - header-name3
          - header-name4 
```

## Serving the folder with files

### Serving from the local folder
```yaml
hosts:
  localhost:8001:
    endpoint:
      type: http
    locations:      
    - proxy_pass_to: ~/web_content
      default_file: index.html
```
default_file - serves with '/' (root) path

### Serving from remote ssh folder

```yaml
hosts:
  localhost:8001:
    endpoint:
      type: http
    locations:      
    - proxy_pass_to: ssh:user@10.0.0.5:22->~/web_content
      default_file: index.html
```


### Variables which can be used to populate headers or content

* ${ENDPOINT_IP} - ip of server listen endpoint;
* ${ENDPOINT_SCHEMA} - http or https schema of listen endpoint;
* ${CLIENT_CERT_CN} - Common name of client certificate if endpoint is protected by client certificate;
* ${PATH_AND_QUERY} - path and query of request;

### Variable tips.
* All the system variables are upper case, and all the custom variables are lower case.


Example of custom variable:
```yaml
variables:
  my_ssh_config: ssh:user@10.12.13.14:22
```

## Types of endpoints

### Http
```yaml
hosts:
  localhost:8000:
    endpoint:
      type: http
```

### Http2
```yaml
hosts:
  localhost:8000:
    endpoint:
      type: http2
```

### Https

Serves http/1.1 over TLS1.3 and TLS1.2

```yaml
hosts:
  localhost:8000:
    endpoint:
      type: https
      ssl_certificate: my_ssl_cert        
```

### Https2

Serves https/2 over TLS1.3 and TLS1.2
Fallbacks to http/1.1 if client does not support http2

```yaml
hosts:
  localhost:8000:
    endpoint:
      type: https2
      ssl_certificate: my_ssl_cert        
```

### Https
```yaml
hosts:
  localhost:8000:
    endpoint:
      type: tcp
```

## Debugging endpoints

Adding debug flag to endpoint will print all the traffic errors to the console

```yaml
  localhost:8000:
    endpoint:
      type: http
      debug: true
```       
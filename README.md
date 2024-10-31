# Configuration Example

File should be at `~/.my-reverse-proxy` location with yaml format:

```yaml
global_settings:
  connection_settings:
    buffer_size: 512Kb # Buffer, which is allocated twice (read/write) per connection to pass traffic by
    connect_to_remote_timeout: 5s # Timeout to connect to remote host
    session_key: # key to encrypt session data. Not having this field means that key is going to be randomly generated
  
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
    revocation_list: ~/certs/revocation_list.crl
    
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


### Serving static content

Example of serving static content with custom headers and body

```yaml
  7700:
    endpoint:
      type: http

    locations:
    - proxy_pass_to: static
      status_code: 200
      content_type: text/html
      body: <h2>Body H2</h2><h3>Body H3</h3>
```


Example of serving redirect to the same url but with https schema

```yaml
  7700:
    endpoint:
      type: http

    locations:
    - proxy_pass_to: static
      status_code: 302
      modify_http_headers:
        add:
          response:
          - name: Location
            value: https://${HOST_PORT}${PATH_AND_QUERY}
```


### System Variables which can be used to populate headers or content

* ${ENDPOINT_IP} - ip of server listen endpoint;
* ${ENDPOINT_SCHEMA} - http or https schema of listen endpoint;
* ${CLIENT_CERT_CN} - Common name of client certificate if endpoint is protected by client certificate;
* ${PATH_AND_QUERY} - path and query of request;
* ${HOST_PORT} - host and port of request;

### Environment variables

As well variables can be read from environment variables

Priory of reading is:
* System variables;
* Yaml variables
* Environment variables


### Variable tips.
* All the system variables are upper cased;
* All the environment variables are upper cased;
* All the custom variables are lower case;


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
hosts:
  localhost:8000:
    endpoint:
      type: http
      debug: true
```       

## Settings up SSH tunnels.

By default if there is no settings for SSH tunnel - SSH agent is used.

### To use password please specify
```yaml
ssh:
  ssh_user@10.0.0.5:
    password: password
```


### To use private key please specify
```yaml
ssh:
  ssh_user@10.0.0.5:
    private_key_file: ~/certs/private_key.key
    passphrase: passphrase
```



## Google OAuth authentication

It is possible to use Google OAuth authentication for the endpoints.

```yaml
hosts:
  localhost:8000:
    endpoint:
      type: https
      ssl_certificate: my_ssl_cert  
      google_auth: g_auth_id

g_auth:
  g_auth_id:
    client_id: ...
    client_secret: ...
    whitelisted_domains: domain1.com;domain2.com
```

If 'whitelisted_domains' property is missing - any email from any domain passed thought google authentication is allowed.



## IP Whitelisting

It's possible to IP whitelist and given endpoint

```yaml
hosts:
  localhost:8000:
    endpoint:
      type: http
      whitelisted_ip: 10.0.0.0;20.0.0.0      
```

or 

```yaml
hosts:
  localhost:8000:
    endpoint:
      type: http
      whitelisted_ip: 10.0.0.0-10.0.0.5;15.0.0.0     
```

Same rules can be applied to any location

```yaml
hosts:
  localhost:443:
    endpoint:
      type: https

    locations:
    - proxy_pass_to: http://10.0.0.4:7702
      whitelisted_ip: 10.0.0.0 
```



## Endpoint templates 

If several endpoints have the same configuration it is possible to use templates

```yaml
hosts:
  domain.com:443:
    endpoint:
      type: https
      template_id: endpoint_template_id


endpoint_templates:
  endpoint_template_id:
    ssl_certificate: ssl_cert_id
    google_auth: google_auth_id
    whitelisted_ip: 10.0.0.0
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



## Allowed users list

It is possible to specify allowed users list for the endpoints which has authentication

```yaml
hosts:
  domain.com:443:
    endpoint:
      type: https
      allowed_users: list_id

allowed_users:
  list_id: 
  - email1@domain.com
  - email2@domain.com
  - email3@domain.com

```

Allowed users can be located in remote file. To specify remote file please use next example:

```yaml
hosts:
  domain.com:443:
    endpoint:
      type: https
      allowed_users: list_id

allowed_users:
  from_file: 
  - http://remote_host:5123/allowed_users_list.yaml
  - ~/allowed_users_list.yaml
  - root@127.0.0.1->~/allowed_users_list.yaml

```

In this case - allowed_users configuration with id='list_id' must be located inside of one of remote files specified in yaml.



### Compressing the http body
Sometimes if proxy pass is done to remote endpoint by ssh - it would be wise to compress http body

```yaml


  8005:
    endpoint:
      type: http2  

    locations:       
    - path: /service1
      type: http2  
      compress: true

```
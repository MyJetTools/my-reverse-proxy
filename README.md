# Configuration Example

File should be at `~/.my-reverse-proxy` location with yaml format:

```yaml
global_settings:
  connection_settings:
    buffer_size: 512Kb # Buffer, which is allocated twice (read/write) per connection to pass traffic by
    connect_to_remote_timeout: 5s # Timeout to connect to remote host
    session_key: # key to encrypt session data. Not having this field means that key is going to be randomly generated
    show_error_description_on_error_page: true # Show error description on error page
  
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
    - type: static
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
    - type: static
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

#### WebSocket support on Http2 / Https2

`Http2` and `Https2` endpoints accept WebSocket connections from the client
side. Supported endpoint × upstream combinations:

| Client endpoint | Upstream location type | Works |
|-----------------|------------------------|-------|
| `https2`        | `http`                 | yes   |
| `https2`        | `http2`                | yes   |
| `https1`        | `http`                 | yes   |
| `http2`         | `http` / `http2`       | yes (non-browser h2 clients only) |

Notes:
- Browsers do WebSocket over HTTP/2 only on TLS — use `https2` for
  browser-initiated WebSocket.
- For an upstream of type `http2` (or `unix+http2`) the upstream service must
  itself support WebSocket-over-HTTP/2. Services built on `MyHttpServer`
  support it from version `0.8.3`.

### Tcp
```yaml
hosts:
  localhost:8000:
    endpoint:
      type: tcp
```

### Mcp (Model Context Protocol)
MCP endpoints provide TCP forwarding with enhanced debugging capabilities for Model Context Protocol connections. They forward raw TCP traffic to remote MCP servers.

**Basic Configuration:**
```yaml
hosts:
  localhost:8000:
    endpoint:
      type: mcp
      debug: true  # Optional: enables debug logging for MCP connections
    locations:
    - proxy_pass_to: remote_host:5123
```


#### MCP endpoints examples
```yaml
hosts:
  mcp.domain.com:443:
    endpoint:
      type: mcp
    locations:
    - path: /mcp_service_1
      proxy_pass_to: http://internal_mcp_server:8100/internal_path

    - path: /mcp_service_2
      proxy_pass_to: http://internal_mcp_server:8101/internal_path_2

```


**Remote Host Configuration:**
The `proxy_pass_to` field supports different formats:

1. **Direct TCP connection:**
```yaml
locations:
- proxy_pass_to: remote_host:5123
```

2. **With HTTP prefix (automatically stripped):**
```yaml
locations:
- proxy_pass_to: http://remote_host:5123
```

3. **SSH tunneling:**
```yaml
locations:
- proxy_pass_to: ssh:username@ssh_host:22->remote_host:5123
```

**Note:** HTTPS (`https://`) is not supported as a remote host format for MCP endpoints.

**MCP Endpoint Features:**
- TCP connection forwarding to remote MCP servers
- Debug logging to monitor MCP protocol traffic
- Support for SSH tunneling: `ssh:user@host:port->remote_host:port`
- Connection timeout and buffer management
- Bidirectional data streaming with detailed logging
- Optional TLS encryption when SSL certificate is provided

**Debug Mode:**
When `debug: true` is enabled, MCP endpoints will log:
- Connection establishment details
- Bidirectional data flow with markers:
  - `->To MCP Server->` for client-to-server traffic
  - `<-From MCP Server<-` for server-to-client traffic
- Protocol message content (text messages shown as strings, binary as length)
- Connection errors and graceful shutdowns
- Connection ID tracking for multiple concurrent connections

**Connection Settings:**
- **Read Timeout**: 3 minutes (180 seconds) - maximum time to wait for data from either side
- **Write Timeout**: 30 seconds - maximum time to wait for write operations
- **Buffer Size**: 512 KB per connection (allocated for read operations)

**Example: MCP Endpoint with SSH Tunneling:**
```yaml
hosts:
  localhost:8443:
    endpoint:
      type: mcp
      ssl_certificate: my_ssl_cert  # Optional: for TLS encryption
      debug: true
    locations:
    - proxy_pass_to: ssh:user@bastion.example.com:22->mcp-server.internal:5123

ssh:
  user@bastion.example.com:
    private_key_file: ~/.ssh/id_rsa
    passphrase: optional_passphrase
```

**Example: Multiple MCP Endpoints:**
```yaml
hosts:
  localhost:8443:
    endpoint:
      type: mcp
      ssl_certificate: mcp_cert  # Optional: for TLS encryption
      debug: true
    locations:
    - proxy_pass_to: mcp-server-1:5123

  localhost:8444:
    endpoint:
      type: mcp
      # No SSL certificate - plain TCP connection
      debug: false  # Disable debug for production
    locations:
    - proxy_pass_to: mcp-server-2:5123
```

**Troubleshooting:**
- If connections fail, check that the remote MCP server is accessible
- Enable `debug: true` to see detailed connection and data flow information
- Check firewall rules for both the listening port and remote host port

## Location types

None tls connections can not infer which type of HTTP protocol endpoint supports (HTTP/1.1 or HTTP2). For this reason it is possible to specify type of location explicitly.

### Http

Used for http/1.1 connections

```yaml
localhost:8001:
    endpoint:
      type: http
    locations:      
    - type: http
      proxy_pass_to: http://remote_host:5123
```

### Http2

Used for http/2 connections

```yaml
localhost:8001:
    endpoint:
      type: http
    locations:      
    - type: http2
      proxy_pass_to: http://remote_host:5123
```

### Unix+Http

Used for http/1.1 connections through a Unix domain socket.

Unix socket path must start with `/` or `~`. A leading `~/` is expanded to
`$HOME/…` at connect time, so configs are portable across users.

```yaml
localhost:8001:
    endpoint:
      type: http
    locations:
    - type: unix+http
      proxy_pass_to: /var/run/myapp.sock

    - type: unix+http
      proxy_pass_to: ~/sockets/myapp.sock     # expands to $HOME/sockets/myapp.sock
```

### Unix+Http2

Used for http/2 connections through a Unix domain socket. Same path rules as
`unix+http` (leading `~/` expands to `$HOME/…`).

```yaml
localhost:8001:
    endpoint:
      type: http
    locations:
    - type: unix+http2
      proxy_pass_to: /var/run/myapp.sock

    - type: unix+http2
      proxy_pass_to: ~/sockets/myapp.sock
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

## Auto-injected request headers

Regardless of `modify_http_headers` configuration, the reverse-proxy always injects
the following headers into proxied HTTP/1.1 and HTTP/2 requests:

* **`X-Forwarded-For`** — appends the incoming client IP per RFC 7239. If the
  request already carries `X-Forwarded-For`, the client IP is appended to the
  existing value (`{existing},{client_ip}`). No header is added for Unix socket
  clients. This cannot be disabled.

* **`CF-IPCountry`** — only when `inject_country: true` is set on the endpoint.
  The 2-letter ISO country code of the client IP is looked up from the bundled
  IPv4 geolocation database and written as `CF-IPCountry: XX`. Any client-supplied
  `CF-IPCountry` header is overwritten. If the IP cannot be resolved to a
  country, the header is not added.

```yaml
hosts:
  localhost:8000:
    endpoint:
      type: http
      inject_country: true
```

The IPv4 geolocation database is compiled into the binary as a static lookup
table. IPv6 lookups use an IP2Location LITE DB1 `.BIN` file
(`ip_v6/IP2LOCATION-LITE-DB1.IPV6.BIN`) loaded via mmap at startup; the file
is shipped alongside the binary in the Docker image. If the `.BIN` file is
missing, IPv6 clients simply do not receive the header.


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
      whitelisted_ip: id_of_ip_list
      
      
ip_white_lists:
  id_of_ip_list:
    - "10.0.0.5"
    - "10.0.0.1-10.0.0.5"
```

Same rules can be applied to any location

```yaml
hosts:
  localhost:443:
    endpoint:
      type: https

    locations:
    - proxy_pass_to: http://10.0.0.4:7702
      whitelisted_ip: id_of_ip_list 
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




### Timeouts for Remote HTTP Endpoints
Remote HTTP endpoints have default timeouts: 5 seconds for establishing a connection and 15 seconds for completing a request.

You can adjust these timeouts using the following configuration example. Values are specified in milliseconds (ms); for instance, 1000 represents 1 second.

```yaml

  8989:
    endpoint:
      type: http

    locations:
      - path: /
        proxy_pass_to: http://127.0.0.1:8080
        connect_timeout: 1000  # 1 second connection timeout
        request_timeout: 2000  # 2 second request timeout

```

# GATEWAY

Two or more instances of reverse proxy can be connected as network and forward traffic through gateway.

How to setup Server Gateway connection
```yaml
gateway_server:
  port: 30000
  encryption_key: 12345678901234567890
```

How to setup Client Gateway connections
```yaml
gateway_clients:
  gateway_name:
    remote_host: 10.0.0.0:30000
    encryption_key: 12345678901234567890
    connect_timeout_seconds: 5
    compress: true 
    allow_incoming_forward_connections: true
    sync_ssl_certificates:
      - my_ssl_cert
      - another_ssl_cert
```

- allow_incoming_forward_connections - is optional. Without this parameters - no Forward connections are allowed through gateway from Client side to Server side.
- sync_ssl_certificates - is optional. List of SSL certificate ids to pull from the remote Gateway Server. See the `SSL Certificate Sync via Gateway` section below.


To Setup location through gateway
```yaml
hosts:
  7777:
    endpoint:
      type: tcp
    locations:
    - proxy_pass_to: gateway:gateway_name->192.168.1.1:5123

  5203:
    endpoint:
      type: http
    locations:
    - proxy_pass_to: gateway:gateway_name->http://localhost:8000
      allow_incoming_forward_connections: true
```



encryption_key - is mandatory and recommended to be 48 symbols and random as possible

allow_incoming_forward_connections  - is optional. Without this parameters - no Forward connections are allowed through gateway from Server side to Client side.



## SSL Certificate Sync via Gateway

SSL certificates can be distributed centrally from the Gateway Server to one or more Gateway Clients. The server stores and renews certificates as usual; clients request the ones they need by id through the Gateway channel and keep them in memory.

### How it works

1. The Gateway Server holds the source of truth: certificates are loaded from `ssl_certificates:` (local files, SSH, etc.) the same way as a standalone proxy.
2. Each Gateway Client lists the certificate ids it wants in `sync_ssl_certificates`.
3. Right after the handshake, the client sends a `SyncSslCertificatesRequest` with its list. For each requested id the server replies either with a `SyncSslCertificates` packet (if it has that cert) or with `SyncSslCertificateNotFound` (if it does not). The client caches received certs in-memory, overwriting any existing entry with the same id.
4. A timer on the client wakes up every 60 seconds, checks the cached sync-origin certificates, and re-requests any id that is missing or has 1 day or less before expiry. Fresh certs on the server are therefore picked up automatically without a reconnect.
5. If the server responds with `SyncSslCertificateNotFound` for a previously cached gateway-pushed cert, the client removes it from its cache. Local (non-gateway-pushed) certificates with the same id are never touched — the client's own `ssl_certificates:` entries are safe. If the server restarts and a cert hasn't finished loading when a request arrives, the client temporarily loses it and then re-fetches it on the next 60s tick — so transient gaps self-heal in under a minute.
6. On client reconnect the full list is requested again, so transient disconnects never leave the client out of date.

The certificate with id `self_signed` is never pushed over the wire — self-signed certs are generated locally on demand.

### Server side

No extra configuration is required on the server beyond the usual `ssl_certificates:` block and a running `gateway_server`:

```yaml
gateway_server:
  port: 30000
  encryption_key: 12345678901234567890

ssl_certificates:
  - id: my_ssl_cert
    certificate: ~/certs/cert.cer
    private_key: ~/certs/cert.key
  - id: another_ssl_cert
    certificate: ~/certs/another_cert.cer
    private_key: ~/certs/another_cert.key
```

### Client side

Add `sync_ssl_certificates` to the Gateway Client entry with the list of certificate ids that should be pulled from the server. The ids must match those defined on the server.

```yaml
gateway_clients:
  gateway_name:
    remote_host: 10.0.0.0:30000
    encryption_key: 12345678901234567890
    sync_ssl_certificates:
      - my_ssl_cert
      - another_ssl_cert

hosts:
  localhost:443:
    endpoint:
      type: https
      ssl_certificate: my_ssl_cert
    locations:
    - type: http
      proxy_pass_to: http://backend:5123
```

The client does not need its own `ssl_certificates:` entry for ids listed in `sync_ssl_certificates` — HTTPS listeners resolve the certificate from the in-memory cache that the gateway sync populates. If both a local `ssl_certificates:` entry and a pushed cert share an id, the pushed one overwrites the local one on each sync.

On the console the client prints a line for every cert it receives, e.g.:

```
received ssl_certificate 'my_ssl_cert' from gateway [gateway_name]
```

And when the server reports a cert as missing, the client removes its cached copy:

```
removed ssl_certificate 'my_ssl_cert' — not present on gateway [gateway_name]
```

### When to use it

- One or more edge proxies (clients) need to terminate TLS for domains whose certificates are managed centrally (Let's Encrypt renewals, rotation, compliance).
- You want to avoid distributing private keys via configuration management or SSH to each node.
- Certificates on the server are renewed by the existing `SslCertsRefreshTimer` — clients converge to the new cert within a minute without a restart or reconnect.



## Gateway reliability

The gateway TCP transport uses a race-free single-channel writer internally —
outgoing payloads travel through a bounded `mpsc` channel that carries the bytes
themselves rather than separate "wake the writer" signal events. This eliminates
a class of lost-wakeup bugs that could cause a long-running gateway to silently
stall after extended uptime under heavy traffic. From a configuration standpoint
nothing changes; the improvement is purely internal.



## Include other files

You can split config files into several config files and include them to .my_reverse_proxy file

```yaml
include:
- ~/other_config_file.yaml
- ~/other_config_file2.yaml
```
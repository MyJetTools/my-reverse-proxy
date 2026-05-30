# Configuration Example

File should be at `~/.my-reverse-proxy` location with yaml format:

```yaml
global_settings:
  connection_settings:
    buffer_size: 512Kb # Buffer, which is allocated twice (read/write) per connection to pass traffic by
    connect_to_remote_timeout: 5000 # ms — timeout to connect to remote host (type: tcp endpoints)
    session_key: # key to encrypt session data. Not having this field means that key is going to be randomly generated
    show_error_description_on_error_page: true # Show error description on error page
  default_h2_livness_url: /health # Optional: HTTP path used by the h2 upstream pool supervisor as an active liveness probe (see "HTTP/2 upstream pool" section)
  
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

These can be tuned per-endpoint (under `endpoint:`). All are optional and fall back to the defaults below. Timeouts are in **milliseconds** (integer); the buffer size accepts a raw byte count or a `Kb`/`Mb` suffix (e.g. `512Kb`, `1Mb`).

| Setting             | YAML key            | Default              | Description |
|---------------------|---------------------|----------------------|-------------|
| Read Timeout        | `mcp_read_timeout`  | 1800000 ms (30 min)  | Max time to wait for data from either side. The client→server direction can stay idle for long periods, so this is a generous safety net. |
| Write Timeout       | `mcp_write_timeout` | 30000 ms (30 sec)    | Max time to wait for write operations. |
| Buffer Size         | `mcp_buffer_size`   | 512 KB (`512Kb`)     | Per-connection read buffer. |

```yaml
hosts:
  localhost:8443:
    endpoint:
      type: mcp
      ssl_certificate: my_ssl_cert
      mcp_read_timeout: 600000   # ms — optional, default 1800000 (30m)
      mcp_write_timeout: 15000   # ms — optional, default 30000 (30s)
      mcp_buffer_size: 1Mb       # optional, default 512Kb
    locations:
    - proxy_pass_to: mcp-server:5123
```

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

### Drop

Silently drops the connection for any request matching this location. No
upstream connection is opened, no response template is written — on HTTP/1.1
the proxy just sends `FIN` and closes the TCP socket.

Use it on wildcard or catch-all hosts to refuse bot traffic, IP-literal
scanners, or any request that targets an unknown path you do not want to
respond to. Combine with `path:` to selectively drop a subtree.

```yaml
hosts:
  80:
    endpoint:
      type: http
    locations:
    - path: /.env
      proxy_pass_to: drop

    - path: /wp-admin
      proxy_pass_to: drop

    - proxy_pass_to: static
      status_code: 302
      modify_http_headers:
        add:
          response:
          - name: Location
            value: https://${HOST_PORT}${PATH_AND_QUERY}
```

Equivalent shorthand using the `type` field:

```yaml
locations:
- path: /.env
  type: drop
```

Behavior per protocol:

- **HTTP/1.1** — the request is parsed, the location is matched, then the TCP
  connection is closed without writing anything back to the client. The
  client sees a connection reset / EOF.
- **HTTP/2** — hyper does not allow forcing a TCP shutdown from inside a
  request handler, so the proxy returns an empty `403 Forbidden` body with
  `Connection: close`. Well-behaved h2 clients close the socket after that;
  the connection itself is not torn down by the proxy.

Drop locations:
- never resolve an upstream — there is no `proxy_pass_to: drop://...` form.
- bypass `auth_header`, `google_auth`, and `client_certificate_ca` checks —
  the request is rejected before any of them runs.
- bypass per-domain metrics (`domain_rps`, `*_traffic_*`, `ws_*_*`) when the
  endpoint also satisfies the default strict mode (`track_metrics_by_all_domains`
  unset / false on a wildcard endpoint).

### Dynamic Proxy

A `dynamic` location forwards each request to an upstream URL specified by
the client in the `proxy-to` request header. Useful when one endpoint must
fan out to many different upstreams and the choice is per-request, not
per-config.

```yaml
hosts:
  localhost:8080:
    endpoint:
      type: http
    locations:
    - path: /
      proxy_pass_to: dynamic
      connect_timeout: 5000     # ms — optional, default 5s
      request_timeout: 15000    # ms — optional, default 15s
      allowed_hosts:            # optional whitelist (case-insensitive)
      - api.example.com
      - cdn.example.com
```

Equivalent shorthand using the `type` field:

```yaml
locations:
- path: /
  type: dynamic
```

Request flow:

1. Client sends `proxy-to: https://target.example` (header name is
   case-insensitive) together with the original request.
2. The proxy parses the header value as a base URL, strips the `proxy-to`
   header, rewrites the request `Host` to the target host, and forwards the
   original path + query as-is.
3. The proxy opens a **fresh HTTP/1.1 connection per request** (no pooling)
   using the parsed scheme — `http` / `ws` use plain TCP, `https` / `wss`
   use TLS with the system CA bundle.
4. The upstream response is returned to the client unchanged.

Behavior notes:

- **HTTP version to upstream is always HTTP/1.1**, regardless of the listen
  endpoint type. WebSocket upgrade is not supported in the first version.
- **`allowed_hosts` is optional**. When present, only listed hosts (matched
  case-insensitively against the parsed host) are accepted; anything else
  returns **403**. When absent, any host is allowed — the location behaves
  as an open proxy and the caller is responsible for authentication
  (combine with `auth_header` / `google_auth` / `whitelisted_ip`).
- **No connection reuse** — every request creates and tears down its own
  upstream connection. This is intentional: the target host is unbounded,
  and a per-host pool with a TTL is not worth the complexity here.

Error responses:

| Condition                                | Status |
|------------------------------------------|--------|
| `proxy-to` header missing                | 421 Misdirected Request |
| `proxy-to` value cannot be parsed as URL | 421 Misdirected Request |
| Target host not in `allowed_hosts`       | 403 Forbidden |
| Cannot connect to upstream               | 503 Upstream unavailable |

Example with auth and whitelist:

```yaml
hosts:
  proxy.example.com:443:
    endpoint:
      type: https
      ssl_certificate: my_ssl_cert
    locations:
    - path: /
      proxy_pass_to: dynamic
      auth_header: "Bearer ${env:DYNAMIC_PROXY_TOKEN}"
      allowed_hosts:
      - api.internal
      - storage.internal
```

A request like:

```
GET /v1/users?limit=10 HTTP/1.1
Host: proxy.example.com
Authorization: Bearer ...
proxy-to: https://api.internal
```

is forwarded to `https://api.internal/v1/users?limit=10` with `Host:
api.internal` and without the `proxy-to` header.



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



## Per-location header authentication

Any location can require an `Authorization` header on every incoming
request. If the header is missing or its value does not match exactly,
the proxy returns **401** with the standard `NOT_AUTHORIZED_PAGE` body
and never opens an upstream connection.

Configure with the `auth_header` field on a location. The value is the
**full expected `Authorization` header value**, scheme included
(`Bearer …`, `Basic …`, etc.):

```yaml
hosts:
  mcp.domain.com:443:
    endpoint:
      type: https
      ssl_certificate: my_ssl_cert
    locations:
    - path: /mcp
      type: mcp
      proxy_pass_to: http://internal_mcp:8100/mcp
      auth_header: "Bearer secret-token-123"
```

Behavior:

- The header **name** is matched case-insensitively (`Authorization`,
  `AUTHORIZATION`, `authorization` all work).
- The header **value** is matched byte-for-byte exact, after stripping
  surrounding ASCII whitespace from the request value.
- If `auth_header` is absent or empty in the YAML, the check is a no-op
  and the location accepts any (or no) `Authorization` header.
- The header is **forwarded to the upstream as-is** — defense in depth,
  the upstream may revalidate.
- Missing-header and wrong-value cases both produce the same 401 — the
  proxy does not leak which one is wrong.
- The check runs **before** any endpoint-level auth (`google_auth`,
  `client_certificate_ca`). Both can be combined: header auth gates
  access, then endpoint auth attaches an identity.

The expected value can be supplied via a variable to keep secrets out of
config files:

```yaml
variables:
  mcp_token: "Bearer ${env:MCP_TOKEN}"

hosts:
  mcp.domain.com:443:
    endpoint:
      type: https
      ssl_certificate: my_ssl_cert
    locations:
    - path: /mcp
      type: mcp
      proxy_pass_to: http://internal_mcp:8100/mcp
      auth_header: ${mcp_token}
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
    connect_timeout: 5000 # ms, optional, default 5000
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



## Timeouts & connection tuning — overview

Every timeout/connection knob in the proxy, where it lives, and what it controls.
Read this first to decide which one you actually need; each row links to its
detailed section below.

### Units

**Every timeout is an integer number of milliseconds** — `connect_timeout`,
`request_timeout`, `pool_*`, `mcp_read_timeout`, `mcp_write_timeout`,
`pool_supervisor_interval`, `connect_to_remote_timeout`, and the gateway
`connect_timeout`. So `1000` = 1 second, `60000` = 1 minute. ⚠️ `5` means **5
milliseconds**, not 5 seconds — a common foot-gun.

The only non-millisecond knobs are the **buffer sizes** (`mcp_buffer_size`,
`connection_settings.buffer_size`), which take a raw byte count or a `Kb` / `Mb`
suffix (e.g. `512Kb`, `1Mb`).

### All settings

| Setting | Where (YAML) | Unit | Default | Controls |
|---------|--------------|------|---------|----------|
| `connect_timeout` | `location` | ms | 5000 | TCP/TLS connect to an HTTP upstream (and the pool's connect). |
| `request_timeout` | `location` | ms | 15000 | Time to complete a full HTTP request/response to the upstream. |
| `pool_size` | `location` | int | 5 | Connections held in the h1/h2 upstream pool. |
| `pool_ping_timeout` | `location` | ms | 1000 | Per-probe timeout of the h2 liveness check. **h2 + liveness URL only.** |
| `pool_hot_window` | `location` | ms | 3000 | Idle window after a success during which the h2 probe is skipped. **h2 + liveness URL only.** |
| `mcp_read_timeout` | `endpoint` (`type: mcp`) | ms | 1800000 (30m) | Max idle on either side of an MCP tunnel before it's closed. |
| `mcp_write_timeout` | `endpoint` (`type: mcp`) | ms | 30000 (30s) | Max time for a single MCP write. |
| `mcp_buffer_size` | `endpoint` (`type: mcp`) | bytes/`Kb`/`Mb` | `512Kb` | Per-connection MCP read buffer. |
| `default_h2_livness_url` | `global_settings` | path | (unset) | Enables the h2 active liveness probe (without it, pool recovery is reactive only). |
| `pool_supervisor_interval` | `global_settings` | ms | 10000 (10s) | How often the single supervisor sweeps every pool. |
| `connect_to_remote_timeout` | `global_settings.connection_settings` | ms | 5000 (5s) | Connect timeout for **`type: tcp`** port-forward endpoints. |
| `buffer_size` | `global_settings.connection_settings` | bytes/`Kb`/`Mb` | `512Kb` | Pump buffer for `type: tcp` port-forward endpoints. |
| `connect_timeout` | `gateway_clients.<name>` | ms | 5000 (5s) | Connect timeout for a gateway-client link. |

### Which knob for which symptom

- **HTTP upstream slow to fail when down** → lower `connect_timeout` on the location.
- **Long-running HTTP request being cut off** → raise `request_timeout` on the location.
- **Too few / too many upstream connections** → `pool_size` on the location.
- **Dead h2 upstream detected too slowly** → set `default_h2_livness_url`, then tune `pool_supervisor_interval` (sweep rate) and `pool_hot_window` (how stale a slot must be before it's probed).
- **MCP tunnel dropped on a long-idle client** → raise `mcp_read_timeout` on the mcp endpoint.
- **`type: tcp` forward hangs connecting** → lower `connect_to_remote_timeout` in `global_settings.connection_settings`.
- **Gateway link slow to fail** → `connect_timeout` on the gateway client.

Detailed sections: [Remote HTTP endpoint timeouts](#timeouts-for-remote-http-endpoints),
MCP **Connection Settings** (under the MCP endpoint docs), and
[HTTP/2 upstream pool](#http2-upstream-pool) below.



## HTTP/2 upstream pool

All HTTP/2 upstreams (`http2`, `https2`, `unix+http2`) share a single named pool
per endpoint instead of opening one connection per request or per incoming
connection. This lets the proxy multiplex tens of thousands of concurrent
requests over a fixed-size set of upstream h2 connections.

### Behavior

- **Pool key**: `(scheme, host, port)`. Two locations pointing to the same
  endpoint share the same pool — opening a second `https2://api:443` location
  does not double the upstream FD count.
- **Pool size**: **5** connections per endpoint by default, overridable
  per-location via `pool_size`.
- **Cold start**: the pool is created at config load / hot-reload, and a
  background supervisor immediately starts filling the 5 slots. If a request
  arrives before any slot is connected, the proxy returns **503**
  `Upstream unavailable`.
- **Reactive recovery**: a dead `MyHttp2Client` auto-reconnects on the next
  `do_request` (transient drops self-heal). If the upstream stays down, slots
  remain empty and the supervisor retries on every tick (10s by default).

### Active liveness check (optional)

If `default_h2_livness_url` is set in `global_settings`, the supervisor
periodically issues a `GET` to that path on each connected slot that has been
idle longer than the **hot window**. Statuses in `200..=205` keep the slot;
anything else (including a timeout or hyper error) tears the slot down on the
spot, and the supervisor reconnects it on the next tick. There is no
multi-failure counter — a single failed probe is enough.

This active probe is **HTTP/2 only**. HTTP/1 pools (`http`, `https`,
`unix+http`, `ws`, `wss`) never probe — their supervisor only refills empty
slots — so `pool_ping_timeout` and `pool_hot_window` have no effect on h1
locations (only `pool_size` does).

```yaml
global_settings:
  default_h2_livness_url: /health
```

Pool tuning parameters. Everything below is in **integer milliseconds** (except
`pool_size`). The per-location ones go under the `location` (next to
`connect_timeout` / `request_timeout`); the supervisor tick is global (one timer
sweeps every pool) and lives in `global_settings`.

| Parameter         | YAML key                  | Scope        | Unit | Default | Effective when |
|-------------------|---------------------------|--------------|------|---------|----------------|
| Pool size         | `pool_size`               | location     | int  | 5       | always (h1 + h2) |
| Connect timeout   | `connect_timeout`         | location     | ms   | 5000    | always (also the pool's connect) |
| Ping timeout      | `pool_ping_timeout`       | location     | ms   | 1000    | **h2 only**, and only if `default_h2_livness_url` is set |
| Hot window        | `pool_hot_window`         | location     | ms   | 3000    | **h2 only**, and only if `default_h2_livness_url` is set |
| Health-check tick | `pool_supervisor_interval`| `global_settings` | ms | 10000 | always |

The **hot window** is how long a connection is considered "hot" after its last
successful use — within that window the supervisor skips the liveness probe.

```yaml
hosts:
  8005:
    endpoint:
      type: http2
    locations:
      - path: /
        proxy_pass_to: https2://api.internal:443
        pool_size: 10          # optional, default 5
        pool_ping_timeout: 500 # ms, optional, default 1000
        pool_hot_window: 2000  # ms, optional, default 3000

global_settings:
  default_h2_livness_url: /health
  pool_supervisor_interval: 5000  # ms, optional, default 10000
```

If `default_h2_livness_url` is not set, the supervisor only refills empty slots
and never actively probes — recovery is purely reactive on the next request.

### Settings cases

**1. Minimal — pool with reactive recovery only**

```yaml
hosts:
  8005:
    endpoint:
      type: http2
    locations:
    - type: http2
      proxy_pass_to: http://backend:5123
```

5 connections to `backend:5123` are opened at startup. No active probe; if the
upstream dies, requests fail until the supervisor refills slots on the next
10s tick.

**2. With active liveness probe**

```yaml
global_settings:
  default_h2_livness_url: /health

hosts:
  8005:
    endpoint:
      type: http2
    locations:
    - type: http2
      proxy_pass_to: http://backend:5123
```

Same as above, plus every 10s (the default `pool_supervisor_interval`) the
supervisor sends `GET /health` on each connected slot that has been idle past
the hot window. A single non-`200..=205` response (or a timeout) recreates the
slot.

**3. Two locations on the same upstream → one shared pool**

```yaml
hosts:
  8005:
    endpoint:
      type: http2
    locations:
    - path: /api/v1
      type: http2
      proxy_pass_to: http://backend:5123
    - path: /api/v2
      type: http2
      proxy_pass_to: http://backend:5123
```

`backend:5123` still gets only **5** upstream connections — both paths
multiplex over the same pool.

**4. Unix socket upstream**

```yaml
global_settings:
  default_h2_livness_url: /health

hosts:
  8005:
    endpoint:
      type: http2
    locations:
    - type: unix+http2
      proxy_pass_to: ~/sockets/myapp.sock
```

5 UDS connections to the socket; liveness probe uses `:authority = localhost`.

### Authority handling

For TCP/TLS upstreams the probe `:authority` is the canonical `host:port`. For
`unix+http2` the supervisor uses `localhost` as a placeholder (the upstream
must accept that authority — services built on `MyHttpServer >= 0.8.3` do).

### Telemetry

The `/configuration` admin endpoint exposes `remote_connections` with new keys
for the h2 pools:

- `h2://host:port`
- `h2s://host:port`
- `uds-h2://path`

Each value is the number of currently connected (ready) slots out of 5.

## Include other files

You can split config files into several config files and include them to .my_reverse_proxy file

```yaml
include:
- ~/other_config_file.yaml
- ~/other_config_file2.yaml
```

## Dynamic settings file

A single settings file can be designated as the **dynamic settings file** — a
config fragment that is editable at runtime over MCP and merged into the running
configuration exactly like an `include:` file.

```yaml
dynamic_configurations_file: ~/.my-reverse-proxy-dynamic
```

Behavior:

- It is merged the same way as an `include:` entry — same schema as the main
  `~/.my-reverse-proxy` file. Anything you can put in the main file (hosts,
  variables, ssl_certificates, …) can go here.
- It is **allowed to be missing**. Unlike a regular `include:`, a non-existent
  dynamic settings file is silently skipped (it may not have been written yet),
  so the proxy starts fine before the file exists.
- Leading `~/` is expanded to `$HOME/…`.

### Editing over MCP

The MCP server exposes tools to read and write this file without touching the
main config:

- **`get_dynamic_settings`** — returns the resolved path, whether the file
  exists, and its raw YAML content (empty if it does not exist yet).
- **`set_dynamic_settings`** — full-file replace. The YAML is validated against
  the settings schema before writing; on a parse error nothing is written.
- **`reload_settings`** — applies the written file to the running proxy.

The intended workflow is always: `get_dynamic_settings` → edit the returned
content → `set_dynamic_settings` (full body) → `reload_settings`. Because
`set_dynamic_settings` is a full-file replace, the content you send must already
contain everything you want to keep plus your changes.

### Inspecting what is applied

- **`get_settings`** — what the files declare *now* (a fresh compile).
- **`get_applied_settings`** — what is *currently live* (a snapshot from the
  last successful reload).

Diffing the two shows whether there are pending changes that a
`reload_settings` would apply.
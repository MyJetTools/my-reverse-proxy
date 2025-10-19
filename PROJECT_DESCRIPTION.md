# My Reverse Proxy

This is a **high-performance, feature-rich reverse proxy server** written in Rust. It's designed to handle various types of network traffic forwarding with advanced features like SSH tunneling, SSL/TLS termination, authentication, and gateway networking.

## Core Features

### 1. Multiple Protocol Support
- HTTP/1.1 and HTTP/2
- HTTPS with TLS 1.2/1.3
- Raw TCP connections
- MCP (Model Context Protocol) with debugging
- WebSocket support
- Unix socket connections

### 2. Advanced Routing & Proxying
- Proxy pass to remote HTTP endpoints
- SSH tunnel forwarding (`ssh:user@host:port->remote:port`)
- Gateway-based forwarding between proxy instances
- Static file serving (local and remote via SSH)
- Custom static content with headers and redirects

### 3. Security & Authentication
- SSL/TLS certificate management
- Client certificate authentication with CA validation
- Certificate Revocation List (CRL) support
- Google OAuth 2.0 authentication
- IP whitelisting with CIDR support
- User allowlists for authenticated endpoints

### 4. Network Features
- **Gateway System**: Multiple proxy instances can be connected as a network, forwarding traffic through encrypted gateways
- SSH tunneling with password/key authentication
- Connection pooling and HTTP client management
- Compression support for SSH connections
- Customizable timeouts and buffer sizes

### 5. Configuration & Management
- YAML-based configuration with extensive templating
- Environment variable support
- Dynamic configuration reloading
- Endpoint templates for reusable configurations
- File includes for modular configs

## Architecture

The project is well-structured with clear separation of concerns:

- **`app/`**: Application context, metrics, and port management
- **`configurations/`**: Configuration parsing and validation
- **`http_proxy_pass/`**: HTTP proxy logic and content sources
- **`tcp_gateway/`**: Gateway networking between proxy instances
- **`tcp_listener/`**: TCP connection handling
- **`google_auth/`**: OAuth authentication flows
- **`ssl/`**: SSL/TLS certificate management
- **`settings/`**: Configuration schema definitions

## Key Dependencies

- **Hyper**: HTTP server/client implementation
- **Tokio**: Async runtime
- **Rustls**: TLS implementation
- **Custom libraries**: The project uses several custom libraries from the MyJetTools organization for HTTP server, SSH, encryption, and other utilities

## Use Cases

This reverse proxy is suitable for:

- **Microservices architecture**: Routing traffic to different backend services
- **Secure tunneling**: Exposing internal services through SSH tunnels
- **Load balancing**: Distributing traffic across multiple backends
- **SSL termination**: Handling HTTPS for backend HTTP services
- **Network gateway**: Connecting multiple proxy instances in a mesh
- **Development environments**: Local development with remote service access
- **Security**: Adding authentication and IP restrictions to services

## Configuration

The proxy is configured via a YAML file at `~/.my-reverse-proxy` with support for:

- Multiple host/port combinations
- Different endpoint types (HTTP, HTTPS, TCP)
- Location-based routing with path matching
- SSL certificates and client authentication
- SSH tunnel configurations
- Header modification and custom responses

## Getting Started

1. **Configuration**: Create a YAML configuration file at `~/.my-reverse-proxy`
2. **SSL Certificates**: Configure SSL certificates for HTTPS endpoints
3. **SSH Setup**: Configure SSH credentials for tunnel connections
4. **Run**: Execute the binary to start the reverse proxy server

## Example Configuration

```yaml
global_settings:
  connection_settings:
    buffer_size: 512Kb
    connect_to_remote_timeout: 5s
    show_error_description_on_error_page: true

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
      type: mcp
      debug: true
    locations:
    - proxy_pass_to: remote_mcp_host:5123

ssl_certificates:
  - id: my_ssl_cert
    certificate: ~/certs/cert.cer
    private_key: ~/certs/cert.key

client_certificate_ca:
  - id: ca_id
    ca: ~/certs/ca.cer
    revocation_list: ~/certs/revocation_list.crl
```

## Features in Detail

### Gateway System
Two or more instances of the reverse proxy can be connected as a network and forward traffic through encrypted gateways:

```yaml
# Server Gateway
gateway_server:
  port: 30000
  encryption_key: 12345678901234567890

# Client Gateway
gateway_clients:
  gateway_name:
    remote_host: 10.0.0.0:30000
    encryption_key: 12345678901234567890
    connect_timeout_seconds: 5
    compress: true
    allow_incoming_forward_connections: true
```

### Authentication Options
- **Google OAuth**: Full OAuth 2.0 flow with domain whitelisting
- **Client Certificates**: X.509 certificate-based authentication
- **IP Whitelisting**: Restrict access by IP address or CIDR ranges
- **User Allowlists**: Email-based user access control

### Header Modification
Flexible HTTP header manipulation at global, endpoint, or location levels:

```yaml
modify_http_headers:
  add:
    request:
    - name: x-real-ip
      value: '${ENDPOINT_IP}'
    response:
    - name: custom-header
      value: custom-value
  remove:
    request:
    - unwanted-header
    response:
    - server-header
```

### MCP (Model Context Protocol) Support
The reverse proxy includes specialized support for MCP endpoints, providing TCP forwarding with enhanced debugging capabilities:

```yaml
hosts:
  localhost:8000:
    endpoint:
      type: mcp
      debug: true  # Enables detailed MCP protocol logging
    locations:
    - proxy_pass_to: remote_mcp_server:5123
```

**MCP Endpoint Features:**
- **TCP Connection Forwarding**: Direct TCP passthrough to MCP servers
- **Debug Logging**: Detailed protocol message logging when `debug: true`
- **SSH Tunneling**: Support for SSH-based MCP connections
- **Bidirectional Streaming**: Full duplex data forwarding with monitoring
- **Connection Management**: Automatic connection pooling and timeout handling

**Debug Mode Benefits:**
- Monitor MCP protocol handshakes
- Track bidirectional message flow
- Troubleshoot connection issues
- Analyze protocol compliance

### System Variables
Built-in variables for dynamic configuration:
- `${ENDPOINT_IP}` - IP of server listen endpoint
- `${ENDPOINT_SCHEMA}` - HTTP or HTTPS schema
- `${CLIENT_CERT_CN}` - Common name of client certificate
- `${PATH_AND_QUERY}` - Request path and query
- `${HOST_PORT}` - Host and port of request


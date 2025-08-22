# ADR-0003: Network Filtering Implementation Approach

## Status
Accepted

## Context

We need to implement DNS-based network filtering for cuenv tasks with the following requirements:
- **Unprivileged operation**: Must work without root/CAP_SYS_ADMIN
- **Per-task isolation**: Each task gets independent network restrictions
- **Domain-based filtering**: Block/allow by hostname, not just IP
- **Production ready**: Battle-tested, maintainable approach

## Decision

We will use **network namespaces with DNS proxy filtering** instead of eBPF/XDP approaches.

### Approach: Network Namespaces + DNS Proxy
```
Task Process
    ↓
Network Namespace (isolated)
    ↓  
/etc/resolv.conf → 127.0.0.1:53
    ↓
DNS Proxy (filters by domain)
    ↓
Forward allowed queries to upstream DNS
```

## Options Considered

### 1. XDP + eBPF L7 DNS Parsing ❌
**Pros:**
- Ultimate performance (line-rate packet processing)
- Kernel-level filtering
- Used by Cloudflare, Cilium

**Cons:**
- Requires CAP_NET_ADMIN + CAP_BPF (privileged)
- System-wide impact (affects all traffic)
- Complex to make per-process
- DNS parsing in eBPF is complex and error-prone
- Overkill for our use case

**Verdict:** Too complex and requires privileges we don't have.

### 2. cgroup + eBPF (BPF_PROG_TYPE_CGROUP_SKB) ❌
**Pros:**
- Per-cgroup filtering
- Good performance
- More targeted than XDP

**Cons:**
- Still requires CAP_BPF + CAP_NET_ADMIN
- DNS parsing complexity remains
- L7 inspection in eBPF is challenging

**Verdict:** Privilege requirements make this unsuitable.

### 3. Landlock Network Restrictions ❌
**Pros:**
- Unprivileged
- Already in use for filesystem restrictions
- Simple API

**Cons:**
- Only port-based filtering (no domain filtering)
- Cannot distinguish between different hosts on same port
- Doesn't meet our domain-based requirements

**Verdict:** Insufficient granularity for DNS-based filtering.

### 4. Network Namespaces + DNS Proxy ✅ **CHOSEN**
**Pros:**
- **Unprivileged**: Works with user namespaces (CLONE_NEWUSER + CLONE_NEWNET)
- **Per-task isolation**: Each task gets its own namespace
- **Domain filtering**: Full control over DNS resolution
- **Battle-tested**: Used by Docker, Podman, systemd-nspawn
- **Maintainable**: Standard container technology
- **Debuggable**: Easy to understand and troubleshoot
- **Future-proof**: Foundation for more container features

**Cons:**
- Higher overhead than eBPF (but negligible for our use case)
- Requires namespace support (Linux-only)

**Verdict:** Best balance of functionality, security, and maintainability.

## Implementation Details

### Core Architecture
1. **Namespace Creation**: `unshare(CLONE_NEWUSER | CLONE_NEWNET | CLONE_NEWNS)`
2. **UID/GID Mapping**: Best-effort setup for filesystem access
3. **DNS Proxy**: Simple UDP proxy on 127.0.0.1:53
4. **Domain Filtering**: Parse DNS queries, check against allowlist
5. **Upstream Forwarding**: Forward allowed queries to 8.8.8.8:53

### Privilege Requirements
- **None**: Uses unprivileged user namespaces
- **Fallback**: Graceful degradation when namespaces unavailable
- **Strict Mode**: `CUENV_SECURITY_STRICT=1` to fail instead of warn

### libcontainer Integration (Future)
```toml
# Future enhancement when libcontainer is stable
libcontainer = "0.5"  # Currently has compilation issues
```

We investigated libcontainer (from Youki container runtime) for professional namespace management, but encountered compilation errors due to seccomp library API changes. Once stable, libcontainer would provide:
- Robust namespace lifecycle management
- Proper UID/GID mapping via newuidmap/newgidmap helpers
- Production-grade error handling
- Container runtime integration patterns

## Consequences

### Positive
- ✅ **Security**: Strong per-task network isolation
- ✅ **Usability**: Works unprivileged out-of-the-box  
- ✅ **Maintainability**: Uses standard Linux container technology
- ✅ **Extensibility**: Foundation for future container features
- ✅ **Compatibility**: Follows Docker/Podman patterns

### Negative
- ❌ **Linux-only**: No Windows/macOS support for network filtering
- ❌ **Namespace overhead**: Process isolation cost
- ❌ **DNS proxy complexity**: Additional moving part

### Monitoring
- DNS query logging for debugging
- Namespace creation success/failure metrics
- Fallback to unrestricted mode when namespaces unavailable

## Future Considerations

1. **eBPF Enhancement**: When CAP_BPF becomes available, add optional eBPF acceleration
2. **libcontainer Adoption**: Migrate to libcontainer when compilation issues resolved
3. **Windows Support**: Investigate Windows container job objects for similar isolation
4. **Performance Optimization**: Connection pooling, DNS caching if needed

## References
- [Linux namespaces documentation](https://man7.org/linux/man-pages/man7/namespaces.7.html)
- [User namespaces and unprivileged containers](https://lwn.net/Articles/532593/)
- [Youki libcontainer architecture](https://github.com/containers/youki)
- [Podman rootless networking](https://github.com/containers/podman/blob/main/docs/tutorials/rootless_tutorial.md)
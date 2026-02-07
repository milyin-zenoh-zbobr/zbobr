# Example: Target Repositories Reference

**⚠️ Note:** This file is for documentation purposes only. The orchestrator itself is domain-agnostic and does not use this file.

**For actual repository lists**, see domain projects (e.g., `YoroolGui/copilot-zenoh/repositories.md`).

---

## Format Example

Domain projects should use this format in their `repositories.md`:

```markdown
# Target Repositories

List repositories that this domain project manages:

- https://github.com/org/repo1
- https://github.com/org/repo2
- https://github.com/org/repo3
```

---

## Real-World Example: Zenoh Project

A good example of a domain project repository list:

### Main Project
- https://github.com/eclipse-zenoh/zenoh

### Language Bindings
- https://github.com/eclipse-zenoh/zenoh-c
- https://github.com/eclipse-zenoh/zenoh-cpp
- https://github.com/eclipse-zenoh/zenoh-python
- https://github.com/eclipse-zenoh/zenoh-ts
- https://github.com/eclipse-zenoh/zenoh-java
- https://github.com/eclipse-zenoh/zenoh-kotlin
- https://github.com/ZettaScaleLabs/zenoh-csharp
- https://github.com/ZettaScaleLabs/zenoh-go

### Pure-C Implementation
- https://github.com/eclipse-zenoh/zenoh-pico

### Plugins
- https://github.com/eclipse-zenoh/zenoh-plugin-dds
- https://github.com/eclipse-zenoh/zenoh-plugin-mqtt
- https://github.com/eclipse-zenoh/zenoh-plugin-webserver
- https://github.com/eclipse-zenoh/zenoh-plugin-ros2dds
- https://github.com/eclipse-zenoh/zenoh-plugin-ros1

### Storage Backends
- https://github.com/eclipse-zenoh/zenoh-backend-filesystem
- https://github.com/eclipse-zenoh/zenoh-backend-influxdb
- https://github.com/eclipse-zenoh/zenoh-backend-rocksdb
- https://github.com/eclipse-zenoh/zenoh-backend-s3

---

See [templates/domain-repositories.md](templates/domain-repositories.md) for the template used when creating new domain projects.

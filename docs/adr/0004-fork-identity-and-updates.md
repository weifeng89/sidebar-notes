# 0004: Give the fork its own app identity

The production app is named Sidebar Notes with bundle identifier `com.weifeng89.sidebarnotes`. It retains Stik's MIT license and existing product features, but uses a separate app configuration, library, and release channel. The original Stik notes remain under their existing storage location.

The updater endpoint points to this fork's releases. Automatic updates default to off until this fork has its own signed release metadata and a verified update path. The upstream Stik signing key remains in the configuration only as an inert placeholder for the retained updater feature; before enabling updates, replace it with a key controlled by this fork and test a signed update end to end.

The upstream analytics consent prompt is suppressed for new fork installations. Analytics stays disabled by default; no PostHog key is supplied by this fork.

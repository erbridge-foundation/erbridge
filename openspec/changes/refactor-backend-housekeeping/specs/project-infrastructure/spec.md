# project-infrastructure — delta for refactor-backend-housekeeping

## ADDED Requirements

### Requirement: Backend service lifecycle and request limits

The backend SHALL shut down gracefully on SIGTERM and SIGINT: stop accepting new connections, allow in-flight requests to complete, then exit — a routine deploy MUST NOT sever requests mid-flight. The backend SHALL bound request duration with a timeout layer (30 seconds) so a stalled upstream cannot hold connections open indefinitely. The listen address SHALL be configurable via a `BIND_ADDR` environment variable, defaulting to `0.0.0.0:3000` so existing deployments need no configuration change.

#### Scenario: SIGTERM drains in-flight requests

- **WHEN** the process receives SIGTERM while a request is in flight
- **THEN** that request completes and receives its response, no new connections are accepted, and the process then exits

#### Scenario: A hung request is terminated by the timeout

- **WHEN** a request's handler does not produce a response within the timeout
- **THEN** the connection receives an error response rather than hanging indefinitely

#### Scenario: Default bind address is unchanged

- **WHEN** the backend starts with no `BIND_ADDR` set
- **THEN** it listens on `0.0.0.0:3000` exactly as before

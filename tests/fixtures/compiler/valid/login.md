## Login Constraints

- Access tokens must never be exposed to the browser.
- Sensitive calls must originate from the server.

## Login Flow

1. Resolve tenant by host.
2. Redirect to the provider.
3. Validate callback.
4. Create a server-side session.

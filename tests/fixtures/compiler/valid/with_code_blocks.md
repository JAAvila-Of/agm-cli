## Usage Example

For example, consider the following authentication code:

```rust
fn authenticate(user: &str, pass: &str) -> Result<Token, AuthError> {
    let hash = bcrypt::hash(pass)?;
    db.verify(user, hash)
}
```

## Configuration Rules

The service must be configured with these environment variables:

```bash
export AUTH_SECRET=...
export SESSION_TTL=86400
```

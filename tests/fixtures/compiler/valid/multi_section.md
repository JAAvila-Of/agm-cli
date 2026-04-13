# Authentication Platform

## Security Rules

- Passwords must be hashed with bcrypt.
- Sessions expire after 24 hours.
- API keys must not appear in logs.

## Login Workflow

1. User enters credentials.
2. System validates against directory.
3. Generate session token.
4. Set secure cookie.

## Token Entity

The token contains the following fields:
- user_id
- session_id
- expiration
- scope

## Error Recovery

In case of authentication failure:
- Lock account after 5 failed attempts.
- Send notification to security team.
- Log the attempt with IP address.

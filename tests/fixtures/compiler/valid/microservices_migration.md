# Monolith to Microservices Migration

## Current Architecture Facts

The existing system is a Ruby on Rails monolith deployed as a single process:

- Framework: Rails 7.1 with PostgreSQL 14
- Deployment: Single Heroku dyno with autoscaling (2-8 instances)
- Database: Shared PostgreSQL with 47 tables and 200+ stored procedures
- Cache: Memcached for session and fragment caching
- Background: Sidekiq with Redis for async job processing
- Search: PostgreSQL full-text search (pg_trgm extension)
- Storage: ActiveStorage with S3 backend
- Auth: Devise with OmniAuth for OAuth2 providers
- API: REST with Jbuilder templates, no versioning strategy

## Migration Constraints

These rules govern the migration process and cannot be violated:

- Zero downtime during migration: the monolith must serve traffic until each service is proven stable.
- Data consistency: no data loss is acceptable during any migration step.
- Feature parity: each extracted service must pass the same acceptance tests as the monolith endpoint.
- Rollback capability: every service extraction must be reversible within 30 minutes.
- Shared database period: during transition, the monolith and new services share the database with schema ownership boundaries.
- API contract: new services must expose the same REST contract as the monolith endpoints they replace.
- No big bang: services are extracted one bounded context at a time over 6 months.

## Service Extraction Workflow

For each bounded context being extracted:

1. Identify the bounded context boundaries by analyzing domain events and data access patterns.
2. Create the new service repository with CI/CD pipeline and infrastructure templates.
3. Implement the service API matching the existing monolith endpoint contracts.
4. Set up a strangler fig proxy that routes traffic based on feature flags.
5. Run the new service in shadow mode: receives traffic but responses are discarded.
6. Compare shadow mode responses with monolith responses for 1 week.
7. If comparison shows less than 0.1% divergence, enable canary routing at 5%.
8. Gradually increase traffic to 25%, 50%, 75%, and finally 100%.
9. After 2 weeks at 100%, remove the monolith code for the extracted context.
10. Transfer database table ownership from shared schema to service schema.

## User Service Entity

The first service to extract is the User service with the following model:

- user_id: UUID replacing the sequential integer ID
- email: unique, case-insensitive, max 254 characters
- password_hash: bcrypt with cost factor 12
- display_name: max 100 characters, optional
- avatar_url: S3 presigned URL, optional
- role: enum (customer, merchant, admin, support)
- status: enum (active, suspended, pending_verification, deleted)
- email_verified: boolean
- phone_number: E.164 format, optional
- mfa_enabled: boolean, default false
- mfa_secret: encrypted at rest, nullable
- locale: BCP 47 language tag, default en-US
- timezone: IANA timezone, default UTC
- last_login_at: timestamp
- login_count: integer
- failed_login_attempts: integer, resets on successful login
- locked_until: timestamp, nullable
- created_at: timestamp
- updated_at: timestamp
- deleted_at: timestamp, nullable for soft delete

## Database Migration Strategy

Database migration follows the expand-contract pattern:

1. Expand phase: add new columns and tables alongside existing ones.
2. Dual-write phase: application writes to both old and new schemas.
3. Backfill phase: migrate historical data from old to new schema.
4. Verify phase: compare old and new data for consistency.
5. Contract phase: remove old columns and tables after verification.

```sql
-- Expand: Add UUID column alongside integer ID
ALTER TABLE users ADD COLUMN uuid UUID DEFAULT gen_random_uuid();
CREATE UNIQUE INDEX idx_users_uuid ON users(uuid);

-- Dual-write trigger
CREATE OR REPLACE FUNCTION sync_user_uuid()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.uuid IS NULL THEN
        NEW.uuid = gen_random_uuid();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_users_uuid
    BEFORE INSERT OR UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION sync_user_uuid();

-- Backfill existing rows
UPDATE users SET uuid = gen_random_uuid() WHERE uuid IS NULL;
```

## Event Bus Architecture Decision

We chose Apache Kafka over RabbitMQ for the inter-service event bus:

Reasons for Kafka:
- Event replay capability for rebuilding service state
- Higher throughput for high-volume domains (order events)
- Built-in partitioning for ordered processing per entity
- Long retention enables new services to catch up on historical events

Trade-offs accepted:
- Higher operational complexity than RabbitMQ
- Consumer group management adds coordination overhead
- Message ordering only guaranteed within a partition
- Requires Zookeeper (or KRaft) cluster management

## Error Recovery Procedures

When a service extraction encounters issues:

- If the new service returns 5xx errors above 1% threshold, the strangler fig proxy automatically routes back to monolith.
- If data inconsistency is detected between monolith and service databases, halt dual-write and trigger reconciliation job.
- If the Kafka consumer falls behind by more than 10,000 messages, scale horizontally and alert the migration team.
- If a schema migration fails mid-execution, use the pre-created rollback script within the 30-minute window.

## API Gateway Configuration

The API gateway handles routing between monolith and microservices:

- Routes are configured per-path with feature flag evaluation.
- Health checks run every 10 seconds against each backend.
- Circuit breakers open after 5 consecutive failures with 30-second recovery window.
- Request/response logging captures headers and status codes (no bodies) for debugging.
- Rate limiting applies per-client-id with configurable burst allowance.
- CORS headers are managed centrally at the gateway level.

```yaml
routes:
  - path: /api/v1/users/**
    backend: user-service
    feature_flag: ff_user_service_enabled
    fallback: monolith
    circuit_breaker:
      threshold: 5
      timeout: 30s
    rate_limit:
      requests: 100
      window: 60s
      burst: 20
```

## Monitoring and Observability

During migration, enhanced observability is critical:

- Distributed tracing with OpenTelemetry across monolith and services.
- Custom metrics dashboard comparing monolith vs service latency per endpoint.
- Alerting on response divergence between shadow mode and production.
- Log aggregation in Elasticsearch with structured JSON logging.
- Weekly migration progress report generated from service traffic percentages.

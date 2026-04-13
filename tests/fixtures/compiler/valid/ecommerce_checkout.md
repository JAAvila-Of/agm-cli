# E-Commerce Checkout System

## Payment Security Rules

All payment processing must comply with PCI-DSS Level 1 requirements:

- Raw card numbers must never be stored, logged, or transmitted through our systems.
- Payment tokens are single-use and expire after 15 minutes.
- All payment API calls must use mutual TLS with certificate pinning.
- Failed payment attempts must be rate-limited to 5 per hour per customer.
- Refund operations require dual authorization from finance and engineering.
- Chargeback notifications must trigger automatic fraud review workflows.

## Cart Validation Workflow

Before processing any checkout, the cart must be validated end-to-end:

1. Verify all items are still in stock by querying the inventory service.
2. Re-fetch current prices from the catalog service to prevent stale-price exploits.
3. Validate shipping address against the address verification API.
4. Apply discount codes and verify they haven't expired or exceeded usage limits.
5. Calculate tax based on shipping destination using the tax calculation service.
6. Compute final total including shipping, tax, discounts, and loyalty points.
7. Create an order draft with a 10-minute reservation on all inventory items.

## Order Entity

The order record contains:

- order_id: UUID primary key
- customer_id: UUID foreign key to customers table
- tenant_id: UUID for multi-tenant isolation
- status: enum (draft, pending_payment, paid, processing, shipped, delivered, cancelled, refunded)
- line_items: array of order line items with SKU, quantity, unit price, and discount
- subtotal: decimal calculated from line items
- tax_amount: decimal from tax calculation service
- shipping_cost: decimal from shipping rate API
- discount_amount: decimal from applied promotions
- total: decimal final charge amount
- currency: ISO 4217 code
- shipping_address: embedded address object
- billing_address: embedded address object
- payment_method_token: string from payment gateway
- created_at: timestamp with timezone
- updated_at: timestamp with timezone

## Payment Processing Workflow

The payment flow handles authorization, capture, and reconciliation:

1. Receive the one-time payment token from the client-side SDK.
2. Generate an idempotency key based on order ID and attempt number.
3. Send authorization request to the payment gateway with the token and amount.
4. If authorization succeeds, store the transaction reference and authorization code.
5. If authorization fails with a retriable error, retry up to 3 times with exponential backoff.
6. If authorization fails with a terminal error, release inventory reservation and notify customer.
7. On successful authorization, mark the order as paid and emit an `order.paid` event.
8. Capture is triggered asynchronously when the warehouse confirms shipment.

```rust
pub async fn authorize_payment(
    gateway: &PaymentGateway,
    order: &Order,
    token: &str,
) -> Result<AuthorizationResult, PaymentError> {
    let idempotency_key = format!("auth-{}-{}", order.id, order.attempt);

    let request = AuthorizationRequest {
        amount: order.total,
        currency: order.currency.clone(),
        token: token.to_string(),
        idempotency_key,
        metadata: json!({
            "order_id": order.id,
            "customer_id": order.customer_id,
        }),
    };

    gateway.authorize(&request).await
}
```

## Inventory Reservation

When a checkout begins, inventory must be reserved to prevent overselling:

- Each SKU gets a soft reservation that expires after 15 minutes.
- Reservations use optimistic locking with a version counter on the inventory row.
- If reservation fails due to concurrent update, retry once with fresh stock data.
- Committed reservations (after payment) are permanent until fulfillment or cancellation.
- Expired reservations are cleaned up by a background job running every 5 minutes.

## Shipping Rate Calculation

Shipping rates depend on multiple factors:

1. Determine the fulfillment warehouse closest to the shipping destination.
2. Calculate the total package weight and dimensional weight.
3. Query carrier APIs (FedEx, UPS, USPS) in parallel for rate quotes.
4. Apply free shipping promotions based on order total and customer loyalty tier.
5. Cache the rate quotes for 10 minutes keyed by destination ZIP and package profile.
6. Present the customer with up to 4 options sorted by delivery speed.

## Fraud Detection Rules

The fraud detection engine evaluates orders in real-time:

- Flag orders where billing and shipping countries differ.
- Flag orders with more than $2000 in value from new accounts (under 30 days).
- Flag orders using prepaid or virtual card BINs above $500.
- Flag orders with 3 or more failed payment attempts in the last hour.
- Auto-reject orders from IP addresses on the global block list.
- Require manual review for flagged orders before fulfillment begins.

## Post-Purchase Communication

After checkout completion, the following notifications are sent:

1. Send order confirmation email with order details and estimated delivery.
2. Send SMS with order number and tracking link (if SMS opt-in).
3. Queue the order for the fulfillment team dashboard.
4. Send webhook notification to any integrated third-party systems.
5. Update the customer's order history in the storefront.
6. Award loyalty points based on the purchase amount and tier multiplier.

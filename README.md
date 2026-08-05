# Hanzo Cloud Backend

Production-grade Rust backend for Hanzo AI cloud services with Training-Free GRPO integration.

## Features

- 🔐 **Authentication**: JWT-based auth with bcrypt password hashing
- 🤖 **Multi-Provider Inference**: DeepSeek, OpenAI, Anthropic, DigitalOcean, Local models
- 🧠 **Training-Free GRPO**: Integration with zoo-gym's experience-based optimization
- 💳 **Credit-Based Billing**: Pay-per-use with Stripe integration
- 📊 **Usage Tracking**: Detailed analytics per user and model
- ⚡ **High Performance**: Async Rust with Tokio runtime
- 🔒 **Secure**: Rate limiting, input validation, secure token storage

## Architecture

```
┌─────────────┐
│   Console   │ (Frontend - Next.js)
│  Port 3000  │
└──────┬──────┘
       │
       ├──── Free Tier ──────┐
       │                     │
       │              ┌──────▼──────┐
       │              │   Gateway   │ (Node.js - IP-based rate limiting)
       │              │  Port 3001  │
       │              └─────────────┘
       │
       └──── Paid Tier ─────┐
                            │
                     ┌──────▼──────┐
                     │    Cloud    │ (Rust - Authenticated, GRPO-enabled)
                     │  Port 8001  │ ◄──── You are here!
                     └──────┬──────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
   ┌────▼────┐        ┌─────▼─────┐      ┌─────▼─────┐
   │ DeepSeek│        │  OpenAI   │      │Zoo-Gym    │
   │   API   │        │    API    │      │  GRPO     │
   └─────────┘        └───────────┘      └───────────┘
```

## Quick Start

### Prerequisites

- Rust 1.75+
- PostgreSQL 14+
- Redis 6+
- Python 3.10+ (for zoo-gym integration)

### Installation

1. Clone the repository:
```bash
cd /Users/z/work/hanzo/cloud
```

2. Copy environment file:
```bash
cp .env.example .env
# Edit .env with your API keys and configuration
```

3. Set up the database:
```bash
# Create PostgreSQL database
createdb hanzo_cloud

# Run migrations (done automatically on startup)
```

4. Install Python dependencies for GRPO:
```bash
cd /Users/z/work/zoo/gym
pip install -e .
```

5. Build and run:
```bash
cargo build --release
cargo run --release
```

The server will start on `http://localhost:8001`.

## API Endpoints

### Health Check
```bash
GET /health
```

### Authentication

**Register:**
```bash
POST /v1/auth/register
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "secure_password",
  "name": "John Doe"
}
```

**Login:**
```bash
POST /v1/auth/login
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "secure_password"
}
```

Response:
```json
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "token_type": "Bearer",
  "expires_in": 3600
}
```

### Inference

**Chat Completions:**
```bash
POST /v1/chat/completions
Authorization: Bearer <access_token>
Content-Type: application/json

{
  "model": "deepseek-chat",
  "messages": [
    {"role": "user", "content": "Write a binary search in Python"}
  ],
  "temperature": 0.7,
  "max_tokens": 1000
}
```

**With GRPO Enabled:**
```bash
POST /v1/chat/completions
Authorization: Bearer <access_token>
Content-Type: application/json

{
  "model": "deepseek-chat",
  "messages": [
    {"role": "user", "content": "Write a binary search in Python"}
  ],
  "grpo_enabled": true,
  "groundtruth": "def binary_search(arr, target):\n    left, right = 0, len(arr) - 1\n    ..."
}
```

**List Models:**
```bash
GET /v1/models
Authorization: Bearer <access_token>
```

### GRPO Management

**List Experiences:**
```bash
GET /v1/grpo/experiences
Authorization: Bearer <access_token>
```

**Get Single Experience:**
```bash
GET /v1/grpo/experiences/{id}
Authorization: Bearer <access_token>
```

**Consolidate Experiences:**
```bash
POST /v1/grpo/consolidate
Authorization: Bearer <access_token>
Content-Type: application/json

{
  "user_id": "uuid"
}
```

### Usage & Billing

**Get Usage:**
```bash
GET /v1/usage
Authorization: Bearer <access_token>
```

Response:
```json
{
  "total_requests": 150,
  "total_tokens": 45000,
  "total_cost": 450,
  "by_model": [
    {
      "model": "deepseek-chat",
      "requests": 100,
      "tokens": 30000,
      "cost": 300
    },
    {
      "model": "gpt-4",
      "requests": 50,
      "tokens": 15000,
      "cost": 150
    }
  ]
}
```

**Get Credits:**
```bash
GET /v1/credits
Authorization: Bearer <access_token>
```

Response:
```json
{
  "credits": 1000,
  "currency": "USD"
}
```

## GRPO Integration

Training-Free GRPO enhances responses by learning from experience without parameter updates.

### How It Works

1. **Experience Injection**: Past learnings are injected into prompts
2. **Multi-Rollout Generation**: Generate G (default 5) responses per query
3. **Reward Computation**: Score responses against groundtruth
4. **Semantic Extraction**: LLM extracts insights from best/worst responses
5. **Experience Update**: Add/modify/delete experiences based on insights

### Benefits

- **No Fine-Tuning Required**: Works with frozen base models
- **Continuous Learning**: Improves with usage
- **Cost Effective**: ~500x cheaper than traditional RL
- **Small Data**: Works with 100+ examples
- **Generalizable**: Learns strategic patterns, not specific solutions

## Development

### Running Tests

```bash
cargo test
```

### Database Migrations

```bash
# Create new migration
sqlx migrate add <migration_name>

# Run migrations
sqlx migrate run
```

### Linting & Formatting

```bash
cargo fmt
cargo clippy -- -D warnings
```

## Deployment

### Docker

```bash
docker build -t hanzo-cloud .
docker run -p 8001:8001 --env-file .env hanzo-cloud
```

### Production Configuration

1. **Use strong JWT secret**: Generate with `openssl rand -base64 64`
2. **Enable HTTPS**: Use reverse proxy (Caddy, Nginx)
3. **Set up monitoring**: Prometheus + Grafana
4. **Configure rate limiting**: Redis-based rate limiter
5. **Enable logging**: Structured JSON logs to stdout
6. **Database**: Use connection pooling (max 10-20 connections)

## Environment Variables

See `.env.example` for all configuration options.

## Credits & Cost

- 1 credit = $0.01 USD
- Default initial credits: 1000 ($10)
- Cost calculation: ~1 credit per 100 tokens
- DeepSeek: ~0.5 credits / 100 tokens
- GPT-4: ~3 credits / 100 tokens
- Claude: ~2 credits / 100 tokens

## License

MIT

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md)

## Support

- Documentation: https://docs.hanzo.ai
- Issues: https://github.com/hanzoai/dev/issues
- Discord: https://discord.gg/CJCyAsm9Vr

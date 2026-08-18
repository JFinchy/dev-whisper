// Curated vocabulary bundles a user can adopt in one click on the
// Vocabulary page, on top of the small built-in default list from
// `stt::default_vocabulary()`. These are just starting points — anything
// adopted here still lands in the same editable `vocabulary` list as a
// manually-typed term, so it can be pruned afterward like any other entry.

export type VocabBook = {
  id: string;
  label: string;
  description: string;
  words: string[];
};

const FRONTEND_WORDS = [
  "React", "Vue", "Angular", "Svelte", "SolidJS", "TypeScript", "JavaScript", "JSX", "TSX",
  "useState", "useEffect", "useMemo", "useCallback", "useRef", "useContext", "useReducer",
  "Redux", "Zustand", "MobX", "Recoil", "Jotai", "Vite", "Webpack", "Rollup", "esbuild",
  "Babel", "ESLint", "Prettier", "Tailwind", "styled-components", "emotion", "SASS", "LESS",
  "PostCSS", "Next.js", "Nuxt", "Remix", "Astro", "Gatsby", "SSR", "SSG", "ISR", "hydration",
  "virtual DOM", "shadow DOM", "Web Components", "Storybook", "Chromatic", "Cypress",
  "Playwright", "Jest", "Vitest", "Testing Library", "npm", "yarn", "pnpm", "bun", "monorepo",
  "Turborepo", "Nx", "Lerna", "WebSocket", "GraphQL", "Apollo", "REST", "Axios", "Fetch API",
  "CORS", "service worker", "PWA", "IndexedDB", "localStorage", "sessionStorage", "WebGL",
  "Canvas", "SVG", "Framer Motion", "GSAP", "D3.js", "Three.js", "accessibility", "ARIA",
  "WCAG", "semantic HTML", "flexbox", "CSS Grid", "media query", "responsive design",
  "viewport", "breakpoint", "DOM", "event bubbling", "event delegation", "debounce",
  "throttle", "memoization", "lazy loading", "code splitting", "tree shaking", "bundle size",
  "Lighthouse", "Core Web Vitals", "LCP", "FID", "CLS", "TTFB", "hot module replacement",
  "HMR", "source maps", "polyfill", "transpile", "design tokens", "DaisyUI", "shadcn",
  "Radix UI", "Headless UI", "Material UI", "Chakra UI", "Figma", "design system",
  "component library", "prop drilling", "controlled component", "uncontrolled component",
  "portal", "suspense", "concurrent rendering", "server components", "client components",
  "hydration mismatch", "TanStack Query", "SWR", "React Router", "Vue Router", "Pinia",
  "Vuex", "composables", "directives", "slots", "computed property", "watcher",
  "lifecycle hook", "template ref", "camelCase", "kebab-case", "BEM",
];

const BACKEND_WORDS = [
  "Node.js", "Express", "Fastify", "NestJS", "Django", "Flask", "FastAPI",
  "Ruby on Rails", "Spring Boot", "ASP.NET", "Go", "Golang", "Rust", "Actix", "Axum",
  "PostgreSQL", "MySQL", "MongoDB", "Redis", "SQLite", "DynamoDB", "Cassandra",
  "Elasticsearch", "ORM", "Prisma", "TypeORM", "Sequelize", "SQLAlchemy", "migration",
  "schema", "index", "foreign key", "primary key", "transaction", "ACID", "CAP theorem",
  "sharding", "replication", "load balancer", "reverse proxy", "Nginx", "Apache", "HAProxy",
  "Kubernetes", "Docker", "Docker Compose", "containerization", "microservices", "monolith",
  "service mesh", "Istio", "gRPC", "protobuf", "message queue", "Kafka", "RabbitMQ", "SQS",
  "pub/sub", "event-driven architecture", "CQRS", "event sourcing", "idempotency",
  "rate limiting", "throttling", "circuit breaker", "API gateway", "JWT", "OAuth", "OAuth2",
  "SAML", "SSO", "RBAC", "authentication", "authorization", "middleware",
  "dependency injection", "REST", "RESTful", "GraphQL", "resolver", "mutation", "webhook",
  "cron job", "worker queue", "Celery", "Sidekiq", "BullMQ", "WebSocket", "long polling",
  "server-sent events", "CDN", "DNS", "TLS", "SSL", "HTTPS", "TCP", "UDP", "HTTP/2", "HTTP/3",
  "AWS", "EC2", "S3", "Lambda", "EKS", "ECS", "RDS", "CloudFront", "GCP", "Azure",
  "Terraform", "Ansible", "Pulumi", "infrastructure as code", "CI/CD", "GitHub Actions",
  "GitLab CI", "Jenkins", "CircleCI", "observability", "structured logging", "tracing",
  "metrics", "Prometheus", "Grafana", "Datadog", "OpenTelemetry", "log aggregation",
  "ELK stack", "health check", "liveness probe", "readiness probe", "blue-green deployment",
  "canary release", "rollback", "horizontal scaling", "vertical scaling",
  "connection pooling", "caching", "cache invalidation", "LRU cache", "N+1 query",
  "database indexing", "query optimization", "EXPLAIN plan", "deadlock", "race condition",
  "concurrency", "parallelism", "thread pool", "async/await", "event loop",
  "non-blocking I/O", "backpressure", "memory leak", "garbage collection",
  "environment variable", "secrets manager", "Vault",
];

const FULLSTACK_WORDS = [
  "TypeScript", "monorepo", "tRPC", "GraphQL", "REST", "OpenAPI", "Swagger", "API contract",
  "backend for frontend", "SSR", "hydration", "environment variable", "feature flag",
  "LaunchDarkly", "A/B test", "staging environment", "production environment",
  "environment parity", "Docker", "docker-compose", "Vercel", "Netlify", "Railway",
  "Render", "Fly.io", "Supabase", "Firebase", "Auth0", "Clerk", "NextAuth", "session",
  "cookie", "CSRF", "XSS", "SQL injection", "input validation", "Zod", "Yup",
  "schema validation", "end-to-end test", "integration test", "unit test", "mocking",
  "stubbing", "test coverage", "git rebase", "git merge", "pull request", "code review",
  "pair programming", "technical debt", "refactor", "architecture decision record", "ADR",
  "design doc", "RFC", "sprint", "standup", "retro", "backlog", "story point", "Jira",
  "Linear", "Notion", "Slack", "incident response", "postmortem", "on-call", "SLA", "SLO",
  "uptime", "latency", "throughput", "p50", "p95", "p99", "error rate", "feature branch",
  "trunk-based development", "semantic versioning", "changelog", "package manager",
  "dependency", "peer dependency", "lockfile", "breaking change", "deprecation",
  "backward compatibility", "hot fix", "patch release", "dotenv", "secrets rotation",
  "rate limiter", "idempotency key", "webhook signature", "HMAC", "npm", "kubectl",
  "Hammerspoon", "init.lua", "zshrc", "MCP",
];

const PRODUCT_WORDS = [
  "roadmap", "backlog", "sprint", "epic", "user story", "acceptance criteria", "MVP",
  "minimum viable product", "product-market fit", "north star metric", "OKR", "KPI",
  "retention", "churn", "activation", "engagement", "DAU", "MAU", "cohort analysis",
  "funnel", "conversion rate", "onboarding", "user journey", "persona",
  "ideal customer profile", "wireframe", "prototype", "usability testing", "A/B testing",
  "experiment", "hypothesis", "feature flag", "beta", "general availability",
  "go-to-market", "positioning", "messaging", "value proposition", "competitive analysis",
  "market research", "TAM", "SAM", "SOM", "pricing tier", "freemium", "upsell",
  "cross-sell", "NPS", "net promoter score", "CSAT", "churn rate", "LTV",
  "lifetime value", "CAC", "customer acquisition cost", "unit economics", "burn rate",
  "runway", "product spec", "PRD", "product requirements document", "stakeholder",
  "cross-functional", "RICE score", "MoSCoW", "Kano model", "design sprint", "discovery",
  "validation", "iteration", "release notes", "feature parity", "competitive moat",
  "differentiation", "brand voice", "user research", "qualitative research",
  "quantitative research", "survey", "interview", "focus group", "journey map",
  "empathy map", "jobs to be done", "JTBD", "growth loop", "viral coefficient",
  "network effect", "flywheel", "pivot", "product-led growth", "sales-led growth",
  "enterprise tier", "self-serve", "waitlist", "launch plan", "press release", "embargo",
  "beta tester", "early adopter", "power user", "edge case", "dogfooding",
  "feature request", "customer feedback", "support ticket", "escalation", "triage",
  "prioritization matrix", "dependency mapping", "cross-team alignment",
  "executive summary", "business case", "ROI", "return on investment",
];

export const VOCAB_BOOKS: VocabBook[] = [
  {
    id: "frontend",
    label: "Frontend",
    description: "React/Vue ecosystem, styling, tooling, web performance",
    words: FRONTEND_WORDS,
  },
  {
    id: "backend",
    label: "Backend",
    description: "Servers, databases, infra, observability",
    words: BACKEND_WORDS,
  },
  {
    id: "fullstack",
    label: "Full-Stack",
    description: "Cross-cutting: auth, testing, deployment, process",
    words: FULLSTACK_WORDS,
  },
  {
    id: "product",
    label: "Product",
    description: "PM/growth vocabulary — roadmap, metrics, research",
    words: PRODUCT_WORDS,
  },
];

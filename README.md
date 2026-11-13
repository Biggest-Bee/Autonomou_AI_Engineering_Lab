# Autonomous AI Engineering Lab

A comprehensive desktop application for autonomous AI code generation, refactoring, and training, built with Tauri v2, React, Rust, and Python.

## 🚀 Architecture Overview

### Desktop Application (Tauri v2 + React)
- **Frontend**: Tauri v2 + React with modern component architecture
- **Backend**: Rust host engine with Virtualization Abstraction Layer (VAL)
- **AI Daemon**: Python 3.12+ background service for AST parsing, embeddings, and ML inference
- **Security**: Rate limiting and environment-based configuration management
- **Sandbox**: Native OS Hybrid Sandbox (NOSH) for isolated code execution

### Security System
- **Rate Limiting**: GitHub API rate limiting with authentication tier support
- **Structured Communication**: JSON-based role separation (system/user) for AI interactions
- **Environment Config**: Secrets and settings loaded from environment variables, never hardcoded

### Production Mode
- **File Explorer Coding**: In production mode, AI operates directly in file explorer
- **PowerShell Terminal**: Uses PowerShell as default terminal for Windows production
- **Sandbox Integration**: Native OS Hybrid Sandbox (NOSH) for code execution isolation
- **User Verification**: All commands require explicit user confirmation in production

### Testing Infrastructure
- **Rust Tests**: `cargo test` for Rust backend validation
- **Python Tests**: `pytest` for Python daemon and AI components
- **Frontend Tests**: `vitest` for React component testing
- **Concurrent Execution**: `concurrently "cargo test" "pytest" "vitest run"` for parallel testing

## 🛠️ Installation

### Prerequisites
- Rust 1.77.2+
- Node.js 22 LTS
- Python 3.12+
- PowerShell 7.4+ (Windows)
- WSL2 (Windows) or Hyper-V (Windows/macOS)

### Setup
1. Clone the repository
2. Copy `.env.example` to `.env` and configure your environment variables
3. Install dependencies:
   ```bash
   # Rust dependencies
   cd src-tauri
   cargo build
   
   # Python dependencies
   cd ../python-daemon
   pip install -r requirements.txt
   
   # Frontend dependencies
   cd ../frontend
   npm install
   ```

### Environment Configuration
See `.env.example` for all required environment variables including:
- System prompt configuration
- API keys (GitHub, OpenAI, etc.)
- Firebase configuration (admin only)
- Sandbox and terminal settings
- Production mode flags

## 🔒 Security Features

### Structured Communication
- AI receives JSON-structured messages with role separation (system/user)

### Rate Limiting
- **GitHub API**: Tiered rate limiting (unauthenticated: 10/min, authenticated: 30/min)
- **General API**: Hourly limits based on authentication (60-15,000 requests/hour)
- **Secondary Limits**: Point-based system to prevent spam (900-2,000 points/min)
- **Concurrency Control**: Maximum of 100 concurrent requests

### Environment Variables
Critical values are stored in environment variables:
- `SYSTEM_PROMPT`: The system prompt sent to the AI
- `PRODUCTION_MODE` / `DEFAULT_TERMINAL`: Production-mode settings
- API keys and secrets (never exposed to AI or client-side code)

## 🧪 Testing

### Run All Tests Concurrently
```bash
concurrently "cargo test" "pytest" "vitest run"
```

### Individual Test Suites
```bash
# Rust tests
cd src-tauri && cargo test

# Python tests
cd python-daemon && pytest -v

# Frontend tests
cd frontend && vitest run
```

## 🎯 Production Mode

### File Explorer Coding
In production mode, the AI operates directly in the file explorer rather than in a virtualized environment. This provides:
- Direct file system access for faster development
- Integration with existing development tools
- Simplified workflow for production deployments

### PowerShell Terminal
Production mode uses PowerShell as the default terminal on Windows:
- Enhanced command execution capabilities
- Better integration with Windows system tools
- Improved script execution and automation

### Sandbox Integration
The Native OS Hybrid Sandbox (NOSH) provides:
- Platform-specific isolation (WSL2 on Windows, vfkit on macOS)
- Unified interface through SandboxEngine trait
- Safe command execution in isolated environments

## 🏗️ Architecture Components

### Virtualization Abstraction Layer (VAL)
- **Linux**: KVM/Firecracker with cgroups v2, namespaces, seccomp
- **macOS**: vfkit over Apple Hypervisor.framework
- **Windows**: WSL2/Hyper-V lightweight containers

### Native OS Hybrid Sandbox (NOSH)
- **Unified Interface**: SandboxEngine trait for cross-platform operations
- **Platform Drivers**: WSL2 (Windows), vfkit (macOS), Firecracker (Linux)
- **Safe Execution**: Isolated command execution with proper cleanup

### AI Daemon (Python)
- **AST Parsing**: Tree-Sitter with regex fallback for multiple languages
- **Vector Storage**: Qdrant with in-memory fallback for embeddings
- **ML Inference**: Transformers with CodeGPT integration
- **IPC**: Apache Arrow with JSON fallback for Rust communication

### Security Layer
- **Rate Limiter**: GitHub API and general request rate limiting
- **Environment Manager**: Secure configuration management

## 📊 Pricing Tiers

### Free Tier ($0/month)
- No sandbox isolation
- Autonomous mode disabled
- Basic AST parsing
- In-memory vector storage

### Pro Tier ($20/month)
- Basic microVM sandbox
- Autonomous mode available
- Full AST parsing
- Qdrant vector database
- ML inference capabilities

### Dev Tier ($50/month)
- Full VAL with multiple microVMs
- Network simulation and chaos engineering
- Advanced security features
- Priority support
- Content-addressable storage

## 🔧 Development

### Project Structure
```
autonomous-ai-engineering-lab/
├── src-tauri/           # Rust backend and Tauri app
│   ├── src/
│   │   ├── val/        # Virtualization Abstraction Layer
│   │   ├── security/   # Security and rate limiting
│   │   ├── sandbox/    # Native OS Hybrid Sandbox
│   │   └── lib.rs      # Main application logic
│   └── Cargo.toml
├── python-daemon/       # Python AI daemon
│   ├── ast_engine.py   # AST parsing
│   ├── vector_store.py # Vector database
│   ├── ml_inference.py # ML inference
│   ├── ipc.py          # IPC communication
│   └── main.py         # Daemon entry point
├── frontend/           # React frontend
│   ├── src/
│   └── package.json
└── .env.example        # Environment configuration template
```

### Build Commands
```bash
# Development
cd src-tauri && cargo tauri dev

# Production build
cd src-tauri && cargo tauri build

# Run Python daemon
cd python-daemon && python main.py

# Start frontend
cd frontend && npm run dev
```

## 📝 Recent Updates

### Security Enhancements ✅
- ✅ Added GitHub API rate limiting with authentication tiers
- ✅ Implemented structured JSON format for AI role separation
- ✅ Environment-based configuration management

### Production Mode ✅
- ✅ Changed production mode to file explorer coding
- ✅ Added PowerShell as default terminal for Windows production
- ✅ Implemented Native OS Hybrid Sandbox (NOSH) architecture
- ✅ Added unified SandboxEngine trait with platform-specific drivers

### Testing Infrastructure ✅
- ✅ Added pytest for Python daemon testing
- ✅ Added vitest for frontend component testing
- ✅ Implemented concurrent test execution with `concurrently`
- ✅ Added unified test command for all frameworks

### Environment Configuration ✅
- ✅ Created comprehensive `.env.example` with all security variables
- ✅ Added API key placeholders for various services
- ✅ Implemented production mode configuration
- ✅ Added sandbox and terminal configuration options

### Stage 1 Foundation ✅ COMPLETED
- ✅ Real VM execution with proper lifecycle management
- ✅ Enhanced PTY handling with process tracking
- ✅ Network isolation infrastructure
- ✅ External tool validation and checking
- ✅ Process lifecycle tracking and cleanup
- ✅ Resource governance with cgroups support

### Stage 2 AI Engine ✅ COMPLETED
- ✅ Incremental AST updates with sub-20ms targets
- ✅ Repository-wide symbol graph construction
- ✅ Real Arrow IPC with proper schema management
- ✅ Comprehensive test coverage for all components
- ✅ Dependency compatibility and conflict resolution
- ✅ Enhanced error handling and runtime validation

### Stage 3 Cloud Integration ✅ COMPLETED
- ✅ Firebase authentication (server-side only)
- ✅ Firestore integration for data persistence
- ✅ Content-addressable storage (CAS) implementation
- ✅ Project sharing and marketplace features
- ✅ Cloud GPU training pipeline with cost estimation

## 🚧 Current Status

### Stage 1: Foundation ✅
- Virtualization Abstraction Layer (VAL) framework
- Terminal session management with WebSocket streaming
- Platform-specific network routing helpers
- Network chaos engineering infrastructure
- Basic PTY/process spawning

### Stage 2: AI Engine ✅
- Tree-Sitter AST parsing with regex fallback
- Sentence-transformer embeddings with Qdrant integration
- Apache Arrow IPC with JSON fallback
- ML inference with CodeGPT integration
- Enhanced daemon IPC handlers

### Security Layer ✅
- GitHub API rate limiting
- Environment-based configuration

### Sandbox Architecture ✅
- Native OS Hybrid Sandbox (NOSH)
- Platform-specific drivers (WSL2, vfkit, Firecracker)
- Unified SandboxEngine trait
- Tauri IPC integration

### Testing Infrastructure ✅
- pytest for Python components
- vitest for frontend testing
- Concurrent test execution
- Unified test commands

## 🎯 Future Roadmap

### Stage 3: Cloud Integration
- Firebase authentication and Firestore
- Content-addressable storage (CAS)
- Project sharing and marketplace
- Cloud GPU training pipeline

### Stage 4: Advanced Features
- Real execution graph visualization
- Autonomy score calculation
- App verification page
- Mock API service ecosystem
- Advanced LSP integration

### Stage 5: Production Features
- Comprehensive logging and monitoring
- Performance optimization
- User management and permissions
- Advanced security auditing
- Multi-language support

## 📄 License

All Rights Reserved

## 🤝 Contributing

This is a private project. For questions or access requests, please contact the development team.

## 📞 Support

For support, visit https://devin.ai/support or contact the development team directly.
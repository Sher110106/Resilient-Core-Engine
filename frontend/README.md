# ChunkStream Pro - Frontend

Modern React-based web interface for the ChunkStream Pro file transfer system.

## Features

- 📤 **File Upload** - Drag & drop or browse to select files
- 📊 **Real-time Dashboard** - Live statistics for active, completed, and failed transfers
- 📡 **WebSocket Integration** - Automatic real-time updates via WebSocket
- 🎮 **Transfer Controls** - Pause, resume, and cancel transfers
- 📈 **Progress Tracking** - Visual progress bars with detailed statistics
- 🎨 **Modern UI** - Beautiful gradient design with responsive layout

## Getting Started

### Prerequisites

- Node.js 14+ and npm
- ChunkStream Pro backend running on `localhost:3000`

### Installation

```bash
cd frontend
npm install
```

### Development

```bash
npm start
```

Runs the app in development mode at [http://localhost:3001](http://localhost:3001).

The page will reload when you make changes.

### Build

```bash
npm run build
```

Builds the app for production to the `build` folder.

## Architecture

### Components

- **App.js** - Main application component with WebSocket management
- **Dashboard.js** - Statistics display (active, completed, failed)
- **FileUpload.js** - File upload interface with drag & drop
- **TransferList.js** - List of active transfers with controls

### Services

- **api.js** - REST API client for backend communication

### API Integration

The frontend connects to the backend REST API:

- `POST /api/v1/transfers` - Start new transfer
- `GET /api/v1/transfers` - List all transfers
- `GET /api/v1/transfers/:id/progress` - Get transfer progress
- `POST /api/v1/transfers/:id/pause` - Pause transfer
- `POST /api/v1/transfers/:id/resume` - Resume transfer
- `POST /api/v1/transfers/:id/cancel` - Cancel transfer
- `WS /ws` - WebSocket for real-time updates

## Configuration

Set the API URL via environment variable:

```bash
REACT_APP_API_URL=http://localhost:3000 npm start
```

Default: `http://localhost:3000`

## Development Notes

### File Upload Implementation

Currently, the frontend passes file objects to the API. For a production implementation, you would need to:

1. Upload files to a staging area on the server
2. Pass the server file path to the transfer API
3. Implement proper file handling and validation

### WebSocket Fallback

The app includes both WebSocket updates (primary) and REST API polling (fallback) to ensure reliability.

## Browser Support

- Chrome (latest)
- Firefox (latest)
- Safari (latest)
- Edge (latest)

## License

Part of the ChunkStream Pro project.

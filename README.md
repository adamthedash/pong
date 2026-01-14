# Pong Server & Client
Mickey mouse project to learn about networking & client-server applications.  

Client & server use a shared [protocol](./protocol/) crate which defines TCP messages.  

Server is in charge of the true game state & simulation.  
Once two players have connected, clients receive an initial state & regular state updates from the server.  
Clients send commands to the server to control their paddle movement.  
Clients maintain a local game state for rendering.  
Server can host many games at once. When two players have connected a game is started and a new session is created for the next players.  

## Components
### Server
- Lobby (main thead): listens for incomming client connections and spins out new game sessions.  
- Orchestrator: Manages each game session
- Simulation: Handles game state updates
- Broadcast: Periodically sends game state updates out to clients
- Control: Listens for client messages & updates game state accordingly.  

### Client
- Render: Simple Egui/Eframe GUI for rendering the game to the user.  
- Listener: Listens for server messages & updates game state accordingly.  
- Game state: Local copy of the game state.  
- Control: Inteprets user input and sends messages to server

### Protocol
- Frame: TCP frames for client/server messages.
- Connection: TCP connection wrapper to read/write frames.

## TODO
- ~~User inputs to control paddles~~
- Client reconnection
- Game pausing/resuming when a player quits/joins
- Better UI for client - scores, game state/end notifications, pending players, etc.
- Protection against malicious client actions
- TUI for server
- Proper logging for client
- Client-side prediction

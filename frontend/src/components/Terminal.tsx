import React, { useEffect, useRef, useState } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { invoke } from '@tauri-apps/api/core';
import './Terminal.css';

const Terminal: React.FC = () => {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const [isConnected, setIsConnected] = useState(false);

  useEffect(() => {
    if (terminalRef.current && !xtermRef.current) {
      // Initialize xterm.js
      const xterm = new XTerm({
        theme: {
          background: '#1a1a1a',
          foreground: '#e0e0e0',
          cursor: '#007acc',
        },
        fontSize: 14,
        fontFamily: 'Consolas, Monaco, monospace',
      });

      const fitAddon = new FitAddon();
      xterm.loadAddon(fitAddon);
      
      xterm.open(terminalRef.current);
      fitAddon.fit();
      
      xtermRef.current = xterm;
      fitAddonRef.current = fitAddon;

      // Handle window resize
      const handleResize = () => {
        if (fitAddonRef.current) {
          fitAddonRef.current.fit();
        }
      };

      window.addEventListener('resize', handleResize);

      // Welcome message
      xterm.writeln('\x1b[32mAutonomous AI Engineering Lab - Terminal\x1b[0m');
      xterm.writeln('Type a command to execute in the microVM...');
      xterm.writeln('');

      return () => {
        window.removeEventListener('resize', handleResize);
        xterm.dispose();
      };
    }
  }, []);

  const startSession = async () => {
    try {
      const newSessionId = `term-${Date.now()}`;
      await invoke('start_terminal_session', { sessionId: newSessionId });
      setIsConnected(true);
      
      if (xtermRef.current) {
        xtermRef.current.writeln(`\x1b[32mConnected to session: ${newSessionId}\x1b[0m`);
      }
    } catch (error) {
      console.error('Failed to start terminal session:', error);
      if (xtermRef.current) {
        xtermRef.current.writeln(`\x1b[31mFailed to connect: ${error}\x1b[0m`);
      }
    }
  };

  const handleCommand = async (command: string) => {
    if (!isConnected) {
      await startSession();
    }

    if (xtermRef.current) {
      xtermRef.current.writeln(`$ ${command}`);
      
      // Simulate command execution (in production, this would communicate with Rust backend)
      setTimeout(() => {
        xtermRef.current?.writeln(`Executed: ${command}`);
        xtermRef.current?.writeln('');
      }, 100);
    }
  };

  return (
    <div className="terminal-container">
      <div className="terminal-header">
        <h3>Terminal</h3>
        <div className="terminal-controls">
          <button onClick={startSession} disabled={isConnected}>
            {isConnected ? 'Connected' : 'Connect'}
          </button>
          <button onClick={() => setIsConnected(false)}>Disconnect</button>
        </div>
      </div>
      <div ref={terminalRef} className="terminal-content" />
      <div className="terminal-input">
        <input
          type="text"
          placeholder="Enter command..."
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              handleCommand(e.currentTarget.value);
              e.currentTarget.value = '';
            }
          }}
        />
      </div>
    </div>
  );
};

export default Terminal;
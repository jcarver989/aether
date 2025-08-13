import React, { useEffect, useState } from 'react';
import { AppContextProvider } from './components/AppContextProvider';
import { ChatView } from './components/ChatView';
import { ChatInput } from './components/ChatInput';
import { ProviderSettings } from './components/ProviderSettings';
import { StreamingPerformanceMonitor } from './components/chat/StreamingPerformanceMonitor';
import { useAppContext } from './hooks/useAppContext';
import { Button } from './components/ui/button';
import { Settings, Activity } from 'lucide-react';
import { Toaster } from 'react-hot-toast';
import "./App.css";

function AppContent() {
  const { actions } = useAppContext();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [performanceMonitorVisible, setPerformanceMonitorVisible] = useState(false);

  useEffect(() => {
    actions.init();
    // Ensure dark mode is applied to html element
    document.documentElement.classList.add('dark');
  }, [actions]);

  return (
    <div className="flex flex-col h-screen bg-background text-foreground relative font-mono border-3 border-border shadow-terminal-glow">
      {/* Retro scan lines overlay */}
      <div className="absolute inset-0 pointer-events-none opacity-5 bg-gradient-to-b from-transparent via-primary/10 to-transparent animate-scan-lines z-0"></div>
      
      <div className="absolute top-4 right-4 z-50 flex items-center gap-2">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setPerformanceMonitorVisible(!performanceMonitorVisible)}
          className="retro-button h-8 w-8 p-0 text-xs border-2 border-primary/60 hover:border-primary hover:shadow-retro"
          title="Toggle performance monitor"
        >
          <Activity className="h-3 w-3" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setSettingsOpen(true)}
          className="retro-button h-8 w-8 p-0 text-xs border-2 border-primary/60 hover:border-primary hover:shadow-retro"
          title="Settings"
        >
          <Settings className="h-3 w-3" />
        </Button>
      </div>
      
      <div className="flex flex-1 overflow-hidden">
        <main className="flex flex-1 flex-col">
          <ChatView className="flex-1" />
          <ChatInput />
        </main>
      </div>

      <ProviderSettings 
        open={settingsOpen} 
        onOpenChange={setSettingsOpen} 
      />

      <StreamingPerformanceMonitor 
        isVisible={performanceMonitorVisible}
      />
    </div>
  );
}

function App() {
  return (
    <AppContextProvider>
      <AppContent />
      
      <Toaster
        position="top-right"
        toastOptions={{
          duration: 3000,
          style: {
            background: 'hsl(var(--background))',
            color: 'hsl(var(--foreground))',
            border: '1px solid hsl(var(--border))',
          },
          success: {
            iconTheme: {
              primary: 'hsl(var(--primary))',
              secondary: 'hsl(var(--primary-foreground))',
            },
          },
          error: {
            iconTheme: {
              primary: 'hsl(var(--destructive))',
              secondary: 'hsl(var(--destructive-foreground))',
            },
          },
        }}
      />
    </AppContextProvider>
  );
}

export default App;

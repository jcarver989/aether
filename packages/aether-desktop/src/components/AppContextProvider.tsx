import React from 'react';
import { AppContext } from '@/hooks/useAppContext';
import { createStore } from '@/state/store';
import { createAppActions } from '@/state/actions';
import { createTauriChannelFactory } from '@/types/ui';

interface AppContextProviderProps {
  children: React.ReactNode;
}

export const AppContextProvider: React.FC<AppContextProviderProps> = ({ children }) => {
  // Create store and dependencies
  const store = React.useMemo(() => createStore(), []);
  const createChannel = React.useMemo(() => createTauriChannelFactory(), []);
  const actions = React.useMemo(() => createAppActions(store, createChannel), [store, createChannel]);

  const contextValue = React.useMemo(() => ({
    store,
    actions,
    createChannel,
  }), [store, actions, createChannel]);

  return (
    <AppContext.Provider value={contextValue}>
      {children}
    </AppContext.Provider>
  );
};
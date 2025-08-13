import React, { useState, useEffect } from 'react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { useAppContext } from '@/hooks/useAppContext';
import { useSelector } from '@/hooks/useSelector';
import type { LlmProvider, OpenRouterConfig, OllamaConfig, AppConfig } from '@/generated/bindings';
import toast from 'react-hot-toast';

interface ProviderSettingsProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export const ProviderSettings: React.FC<ProviderSettingsProps> = ({ open, onOpenChange }) => {
  const { actions } = useAppContext();
  const config = useSelector(state => state.config);
  
  const [selectedProvider, setSelectedProvider] = useState<LlmProvider>('OpenRouter');
  const [isTestingConnection, setIsTestingConnection] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  
  // OpenRouter config
  const [openRouterApiKey, setOpenRouterApiKey] = useState('');
  const [openRouterModel, setOpenRouterModel] = useState('anthropic/claude-3.5-sonnet');
  const [openRouterBaseUrl, setOpenRouterBaseUrl] = useState('');
  const [openRouterTemperature, setOpenRouterTemperature] = useState('0.7');
  
  // Ollama config
  const [ollamaBaseUrl, setOllamaBaseUrl] = useState('http://localhost:11434');
  const [ollamaModel, setOllamaModel] = useState('llama3.1');
  const [ollamaTemperature, setOllamaTemperature] = useState('0.7');

  // Load config when component mounts or config changes
  useEffect(() => {
    if (config) {
      setSelectedProvider(config.active_provider);
      
      // Load OpenRouter config
      setOpenRouterApiKey(config.openrouter_config.api_key);
      setOpenRouterModel(config.openrouter_config.model);
      setOpenRouterBaseUrl(config.openrouter_config.base_url || '');
      setOpenRouterTemperature(config.openrouter_config.temperature?.toString() || '0.7');
      
      // Load Ollama config
      setOllamaBaseUrl(config.ollama_config.base_url);
      setOllamaModel(config.ollama_config.model);
      setOllamaTemperature(config.ollama_config.temperature?.toString() || '0.7');
    }
  }, [config]);

  const handleTestConnection = async () => {
    setIsTestingConnection(true);
    try {
      const openrouterConfig: OpenRouterConfig = {
        api_key: openRouterApiKey,
        model: openRouterModel,
        base_url: openRouterBaseUrl || null,
        temperature: parseFloat(openRouterTemperature) || null,
      };

      const ollamaConfig: OllamaConfig = {
        base_url: ollamaBaseUrl,
        model: ollamaModel,
        temperature: parseFloat(ollamaTemperature) || null,
      };

      const success = await actions.testProviderConnection(
        selectedProvider,
        selectedProvider === 'OpenRouter' ? openrouterConfig : undefined,
        selectedProvider === 'Ollama' ? ollamaConfig : undefined
      );

      if (success) {
        toast.success('Connection test successful!');
      } else {
        toast.error('Connection test failed. Please check your settings.');
      }
    } catch (error) {
      console.error('Connection test error:', error);
      toast.error('Connection test failed. Please check your settings.');
    } finally {
      setIsTestingConnection(false);
    }
  };

  const handleSaveAndInitialize = async () => {
    setIsSaving(true);
    try {
      const openrouterConfig: OpenRouterConfig = {
        api_key: openRouterApiKey,
        model: openRouterModel,
        base_url: openRouterBaseUrl || null,
        temperature: parseFloat(openRouterTemperature) || null,
      };

      const ollamaConfig: OllamaConfig = {
        base_url: ollamaBaseUrl,
        model: ollamaModel,
        temperature: parseFloat(ollamaTemperature) || null,
      };

      const newConfig: AppConfig = {
        active_provider: selectedProvider,
        openrouter_config: openrouterConfig,
        ollama_config: ollamaConfig,
        mcp_servers: config?.mcp_servers || [],
      };

      // Save config
      await actions.saveConfig(newConfig);

      // Initialize agent with new settings
      await actions.initializeAgent(
        selectedProvider,
        selectedProvider === 'OpenRouter' ? openrouterConfig : undefined,
        selectedProvider === 'Ollama' ? ollamaConfig : undefined
      );

      toast.success('Settings saved and agent initialized!');
      onOpenChange(false);
    } catch (error) {
      console.error('Save error:', error);
      toast.error('Failed to save settings. Please try again.');
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Provider Settings</DialogTitle>
          <DialogDescription>
            Configure your LLM provider settings. The agent will be initialized with these settings.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-6">
          {/* Provider Selection */}
          <div className="space-y-2">
            <Label htmlFor="provider">LLM Provider</Label>
            <Select value={selectedProvider} onValueChange={(value) => setSelectedProvider(value as LlmProvider)}>
              <SelectTrigger>
                <SelectValue placeholder="Select a provider" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="OpenRouter">OpenRouter</SelectItem>
                <SelectItem value="Ollama">Ollama</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* OpenRouter Configuration */}
          {selectedProvider === 'OpenRouter' && (
            <Card>
              <CardHeader>
                <CardTitle>OpenRouter Configuration</CardTitle>
                <CardDescription>
                  Configure your OpenRouter API settings
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="openrouter-api-key">API Key</Label>
                  <Input
                    id="openrouter-api-key"
                    type="password"
                    placeholder="sk-or-..."
                    value={openRouterApiKey}
                    onChange={(e) => setOpenRouterApiKey(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="openrouter-model">Model</Label>
                  <Input
                    id="openrouter-model"
                    placeholder="anthropic/claude-3.5-sonnet"
                    value={openRouterModel}
                    onChange={(e) => setOpenRouterModel(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="openrouter-base-url">Base URL (Optional)</Label>
                  <Input
                    id="openrouter-base-url"
                    placeholder="https://openrouter.ai/api/v1"
                    value={openRouterBaseUrl}
                    onChange={(e) => setOpenRouterBaseUrl(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="openrouter-temperature">Temperature</Label>
                  <Input
                    id="openrouter-temperature"
                    type="number"
                    min="0"
                    max="2"
                    step="0.1"
                    placeholder="0.7"
                    value={openRouterTemperature}
                    onChange={(e) => setOpenRouterTemperature(e.target.value)}
                  />
                </div>
              </CardContent>
            </Card>
          )}

          {/* Ollama Configuration */}
          {selectedProvider === 'Ollama' && (
            <Card>
              <CardHeader>
                <CardTitle>Ollama Configuration</CardTitle>
                <CardDescription>
                  Configure your local Ollama instance
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="ollama-base-url">Base URL</Label>
                  <Input
                    id="ollama-base-url"
                    placeholder="http://localhost:11434"
                    value={ollamaBaseUrl}
                    onChange={(e) => setOllamaBaseUrl(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="ollama-model">Model</Label>
                  <Input
                    id="ollama-model"
                    placeholder="llama3.1"
                    value={ollamaModel}
                    onChange={(e) => setOllamaModel(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="ollama-temperature">Temperature</Label>
                  <Input
                    id="ollama-temperature"
                    type="number"
                    min="0"
                    max="2"
                    step="0.1"
                    placeholder="0.7"
                    value={ollamaTemperature}
                    onChange={(e) => setOllamaTemperature(e.target.value)}
                  />
                </div>
              </CardContent>
            </Card>
          )}

          {/* Action Buttons */}
          <div className="flex justify-between">
            <Button
              variant="outline"
              onClick={handleTestConnection}
              disabled={isTestingConnection}
            >
              {isTestingConnection ? 'Testing...' : 'Test Connection'}
            </Button>
            
            <div className="space-x-2">
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                Cancel
              </Button>
              <Button 
                onClick={handleSaveAndInitialize}
                disabled={isSaving}
              >
                {isSaving ? 'Saving...' : 'Save & Initialize'}
              </Button>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
};
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Label } from './ui/label';
import { Eye, EyeOff, Lock, Unlock, Check, X, Loader2, Save } from 'lucide-react';
import { ModelManager } from './WhisperModelManager';
import { ParakeetModelManager } from './ParakeetModelManager';
import { MeetingDomainSettings } from './MeetingDomainSettings';
import { DictionarySettings } from './DictionarySettings';


export interface TranscriptModelProps {
    provider: 'localWhisper' | 'parakeet' | 'deepgram' | 'elevenLabs' | 'groq' | 'openai' | 'assemblyai' | 'gemini' | 'cartesia' | 'speechmatics';
    model: string;
    apiKey?: string | null;
}

export interface TranscriptSettingsProps {
    transcriptModelConfig: TranscriptModelProps;
    setTranscriptModelConfig: (config: TranscriptModelProps) => void;
    onModelSelect?: () => void;
}

export function TranscriptSettings({ transcriptModelConfig, setTranscriptModelConfig, onModelSelect }: TranscriptSettingsProps) {
    const [apiKey, setApiKey] = useState<string | null>(transcriptModelConfig.apiKey || null);
    const [showApiKey, setShowApiKey] = useState<boolean>(false);
    const [isApiKeyLocked, setIsApiKeyLocked] = useState<boolean>(true);
    const [isLockButtonVibrating, setIsLockButtonVibrating] = useState<boolean>(false);
    const [uiProvider, setUiProvider] = useState<TranscriptModelProps['provider']>(transcriptModelConfig.provider);
    // Tracks the last-saved key so the UI can show whether there are unsaved edits.
    const [savedApiKey, setSavedApiKey] = useState<string | null>(transcriptModelConfig.apiKey || null);
    const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');
    const [saveError, setSaveError] = useState<string | null>(null);
    const [testState, setTestState] = useState<'idle' | 'testing' | 'success' | 'error'>('idle');
    const [testMessage, setTestMessage] = useState<string | null>(null);
    const [registryDetected, setRegistryDetected] = useState<boolean>(false);
    const [activeRegistryCheck, setActiveRegistryCheck] = useState<string | null>(null);

    // Check whether the central NERV key registry already has a key for this
    // provider — drives the "auto-detected, no typing needed" badge (#885).
    const checkRegistry = async (provider: string) => {
        setActiveRegistryCheck(provider);
        try {
            const detected = await invoke<boolean>('registry_has_key', { provider });
            // Only update state if this provider is still the active check
            setActiveRegistryCheck((activeProvider) => {
                if (activeProvider === provider) {
                    setRegistryDetected(detected);
                    return null;
                }
                return activeProvider;
            });
        } catch (e) {
            console.error(`Failed to check registry for provider ${provider}`);
            setActiveRegistryCheck((activeProvider) => (activeProvider === provider ? null : activeProvider));
            setRegistryDetected(false);
        }
    };

    useEffect(() => {
        const p = transcriptModelConfig.provider;
        if (p !== 'localWhisper' && p !== 'parakeet') {
            checkRegistry(p);
        } else {
            setRegistryDetected(false);
        }
    }, [transcriptModelConfig.provider]);

    // Sync uiProvider when backend config changes (e.g., after model selection or initial load)
    useEffect(() => {
        setUiProvider(transcriptModelConfig.provider);
    }, [transcriptModelConfig.provider]);

    useEffect(() => {
        if (transcriptModelConfig.provider === 'localWhisper' || transcriptModelConfig.provider === 'parakeet') {
            setApiKey(null);
        }
    }, [transcriptModelConfig.provider]);

    const fetchApiKey = async (provider: string) => {
        try {

            // Clear API key state immediately when switching providers
            setApiKey(null);
            setSavedApiKey(null);

            setApiKey('');
            setSavedApiKey('');
            const data = await invoke('api_get_transcript_api_key', { provider }) as string;

            setApiKey(data || '');
            setSavedApiKey(data || '');
        } catch (err) {
            console.error('Error fetching API key:', err);
            setApiKey(null);
            setSavedApiKey(null);
        }
        // Reset transient UI feedback when switching providers / reloading the key.
        setSaveState('idle');
        setSaveError(null);
        setTestState('idle');
        setTestMessage(null);
    };

    // Persist the key (and current provider/model) to the backend store.
    const handleSaveApiKey = async (): Promise<void> => {
        const trimmed = (apiKey || '').trim();
        if (!trimmed) {
            setSaveState('error');
            setSaveError('API key is empty');
            return;
        }
        setSaveState('saving');
        setSaveError(null);
        try {
            await invoke('api_save_transcript_config', {
                provider: uiProvider,
                model: transcriptModelConfig.model,
                apiKey: trimmed,
            });
            // Keep the in-memory config in sync so other components see the key.
            setTranscriptModelConfig({ provider: uiProvider, model: transcriptModelConfig.model, apiKey: trimmed });
            setSaveState('saved');
            setIsApiKeyLocked(true);
            // Re-arm the "saved" badge after a moment.
            setTimeout(() => setSaveState((s) => (s === 'saved' ? 'idle' : s)), 2500);
        } catch (err) {
            console.error('Error saving API key:', err);
            setSaveState('error');
            setSaveError(typeof err === 'string' ? err : 'Failed to save API key');
        }
    };

    // Validate the key against the provider without uploading audio.
    const handleTestApiKey = async (): Promise<void> => {
        const trimmed = (apiKey || '').trim();
        if (!trimmed) {
            setTestState('error');
            setTestMessage('Enter an API key first');
            return;
        }
        setTestState('testing');
        setTestMessage(null);
        try {
            const result = await invoke('api_test_transcript_api_key', {
                provider: uiProvider,
                apiKey: trimmed,
            }) as { status?: string; message?: string };
            setTestState('success');
            setTestMessage(result?.message || 'API key is valid');
        } catch (err) {
            console.error('API key test failed:', err);
            setTestState('error');
            setTestMessage(typeof err === 'string' ? err : 'API key validation failed');
        }
    };
    const modelOptions = {
        localWhisper: [], // Model selection handled by ModelManager component
        parakeet: [], // Model selection handled by ParakeetModelManager component
        deepgram: ['nova-2', 'nova-2-phonecall', 'nova-2-meeting', 'nova-2-general'],
        elevenLabs: ['eleven_multilingual_v2', 'eleven_turbo_v2'],
        groq: ['whisper-large-v3-turbo', 'whisper-large-v3', 'distil-whisper-large-v3-en'],
        openai: ['whisper-1', 'gpt-4o-transcribe', 'gpt-4o-mini-transcribe'],
        assemblyai: ['best', 'nano'],
        gemini: ['gemini-2.0-flash'],
        cartesia: ['sonic'],
        speechmatics: ['en', 'multilingual'],
    };
    const requiresApiKey = transcriptModelConfig.provider === 'deepgram' || transcriptModelConfig.provider === 'elevenLabs' || transcriptModelConfig.provider === 'openai' || transcriptModelConfig.provider === 'groq' || transcriptModelConfig.provider === 'assemblyai' || transcriptModelConfig.provider === 'gemini' || transcriptModelConfig.provider === 'cartesia' || transcriptModelConfig.provider === 'speechmatics';

    const handleInputClick = () => {
        if (isApiKeyLocked) {
            setIsLockButtonVibrating(true);
            setTimeout(() => setIsLockButtonVibrating(false), 500);
        }
    };

    const handleWhisperModelSelect = (modelName: string) => {
        setTranscriptModelConfig({
            provider: 'localWhisper',
            model: modelName
        });
        // Close modal after selection
        if (onModelSelect) {
            onModelSelect();
        }
    };

    const handleParakeetModelSelect = (modelName: string) => {
        // Always update config when model is selected, regardless of current provider
        setTranscriptModelConfig({
            provider: 'parakeet',
            model: modelName
        });
        // Close modal after selection
        if (onModelSelect) {
            onModelSelect();
        }
    };

    return (
        <div>
            <div>
                {/* <div className="flex justify-between items-center mb-4">
                    <h3 className="text-lg font-semibold text-gray-900">Transcript Settings</h3>
                </div> */}
                <div className="space-y-4 pb-6">
                    <div>
                        <Label className="block text-sm font-medium text-gray-700 mb-1">
                            Transcript Model
                        </Label>
                        <div className="flex space-x-2 mx-1">
                            <Select
                                value={uiProvider}
                                onValueChange={(value) => {
                                    const provider = value as TranscriptModelProps['provider'];
                                    setUiProvider(provider);
                                    if (provider !== 'localWhisper' && provider !== 'parakeet') {
                                        fetchApiKey(provider);
                                        checkRegistry(provider);
                                    }
                                }}
                            >
                                <SelectTrigger className='focus:ring-1 focus:ring-blue-500 focus:border-blue-500'>
                                    <SelectValue placeholder="Select provider" />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="parakeet">⚡ Parakeet (Recommended - Real-time / Accurate)</SelectItem>
                                    <SelectItem value="localWhisper">🏠 Local Whisper (High Accuracy)</SelectItem>
                                    <SelectItem value="deepgram">☁️ Deepgram</SelectItem>
                                    <SelectItem value="groq">☁️ Groq (Whisper)</SelectItem>
                                    <SelectItem value="openai">☁️ OpenAI</SelectItem>
                                    <SelectItem value="assemblyai">☁️ AssemblyAI</SelectItem>
                                    <SelectItem value="gemini">☁️ Gemini</SelectItem>
                                    <SelectItem value="elevenLabs">☁️ ElevenLabs</SelectItem>
                                    <SelectItem value="cartesia">☁️ Cartesia</SelectItem>
                                    <SelectItem value="speechmatics">☁️ Speechmatics</SelectItem>
                                </SelectContent>
                            </Select>

                            {uiProvider !== 'localWhisper' && uiProvider !== 'parakeet' && (
                                <Select
                                    value={transcriptModelConfig.model}
                                    onValueChange={(value) => {
                                        const model = value as TranscriptModelProps['model'];
                                        setTranscriptModelConfig({ provider: uiProvider, model, apiKey: transcriptModelConfig.apiKey });
                                    }}
                                >
                                    <SelectTrigger className='focus:ring-1 focus:ring-blue-500 focus:border-blue-500'>
                                        <SelectValue placeholder="Select model" />
                                    </SelectTrigger>
                                    <SelectContent>
                                        {modelOptions[uiProvider].map((model) => (
                                            <SelectItem key={model} value={model}>{model}</SelectItem>
                                        ))}
                                    </SelectContent>
                                </Select>
                            )}

                        </div>
                    </div>

                    {uiProvider === 'localWhisper' && (
                        <div className="mt-6">
                            <ModelManager
                                selectedModel={transcriptModelConfig.provider === 'localWhisper' ? transcriptModelConfig.model : undefined}
                                onModelSelect={handleWhisperModelSelect}
                                autoSave={true}
                            />
                        </div>
                    )}

                    {uiProvider === 'parakeet' && (
                        <div className="mt-6">
                            <ParakeetModelManager
                                selectedModel={transcriptModelConfig.provider === 'parakeet' ? transcriptModelConfig.model : undefined}
                                onModelSelect={handleParakeetModelSelect}
                                autoSave={true}
                            />
                        </div>
                    )}

                    {uiProvider === 'localWhisper' && (
                        <div className="mt-6">
                            <MeetingDomainSettings />
                        </div>
                    )}

                    <div className="mt-6">
                        <DictionarySettings />
                    </div>


                    {requiresApiKey && (
                        <div>
                            <Label className="block text-sm font-medium text-gray-700 mb-1">
                                API Key
                            </Label>
                            {registryDetected && (
                                <p className="text-xs text-green-600 mb-1 mx-1 flex items-center gap-1">
                                    <span aria-hidden>✓</span>
                                    Auto-detected from your central key registry — no need to type it.
                                </p>
                            )}
                            <div className="relative mx-1">
                                <Input
                                    type={showApiKey ? "text" : "password"}
                                    className={`pr-24 focus:ring-1 focus:ring-blue-500 focus:border-blue-500 ${isApiKeyLocked ? 'bg-gray-100 cursor-not-allowed' : ''
                                        }`}
                                    value={apiKey || ''}
                                    onChange={(e) => {
                                        setApiKey(e.target.value);
                                        // Editing invalidates any prior test/save feedback.
                                        if (testState !== 'idle') { setTestState('idle'); setTestMessage(null); }
                                        if (saveState === 'saved' || saveState === 'error') { setSaveState('idle'); setSaveError(null); }
                                    }}
                                    disabled={isApiKeyLocked}
                                    onClick={handleInputClick}
                                    placeholder="Enter your API key"
                                />
                                {isApiKeyLocked && (
                                    <div
                                        onClick={handleInputClick}
                                        className="absolute inset-0 flex items-center justify-center bg-gray-100 bg-opacity-50 rounded-md cursor-not-allowed"
                                    />
                                )}
                                <div className="absolute inset-y-0 right-0 pr-1 flex items-center">
                                    <Button
                                        type="button"
                                        variant="ghost"
                                        size="icon"
                                        onClick={() => setIsApiKeyLocked(!isApiKeyLocked)}
                                        className={`transition-colors duration-200 ${isLockButtonVibrating ? 'animate-vibrate text-red-500' : ''
                                            }`}
                                        title={isApiKeyLocked ? "Unlock to edit" : "Lock to prevent editing"}
                                    >
                                        {isApiKeyLocked ? <Lock className="h-4 w-4" /> : <Unlock className="h-4 w-4" />}
                                    </Button>
                                    <Button
                                        type="button"
                                        variant="ghost"
                                        size="icon"
                                        onClick={() => setShowApiKey(!showApiKey)}
                                    >
                                        {showApiKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                    </Button>
                                </div>
                            </div>

                            {/* Action row: Test + Save with explicit status feedback */}
                            <div className="flex items-center gap-2 mx-1 mt-2">
                                <Button
                                    type="button"
                                    variant="outline"
                                    size="sm"
                                    onClick={handleTestApiKey}
                                    disabled={isApiKeyLocked || testState === 'testing' || !(apiKey || '').trim()}
                                    className="flex items-center gap-1.5"
                                    title="Validate the key against the provider without uploading audio"
                                >
                                    {testState === 'testing'
                                        ? <Loader2 className="h-4 w-4 animate-spin" />
                                        : <Check className="h-4 w-4" />}
                                    {testState === 'testing' ? 'Testing…' : 'Test'}
                                </Button>
                                <Button
                                    type="button"
                                    size="sm"
                                    onClick={handleSaveApiKey}
                                    disabled={isApiKeyLocked || saveState === 'saving' || !(apiKey || '').trim()}
                                    className="flex items-center gap-1.5"
                                >
                                    {saveState === 'saving'
                                        ? <Loader2 className="h-4 w-4 animate-spin" />
                                        : <Save className="h-4 w-4" />}
                                    {saveState === 'saving' ? 'Saving…' : 'Save'}
                                </Button>

                                {/* Unsaved-changes hint */}
                                {!isApiKeyLocked && (apiKey || '').trim() !== (savedApiKey || '').trim() && saveState === 'idle' && (
                                    <span className="text-xs text-amber-600">Unsaved changes</span>
                                )}
                            </div>

                            {/* Save status */}
                            {saveState === 'saved' && (
                                <p className="flex items-center gap-1 mx-1 mt-1.5 text-xs text-green-600">
                                    <Check className="h-3.5 w-3.5" /> API key saved
                                </p>
                            )}
                            {saveState === 'error' && saveError && (
                                <p className="flex items-center gap-1 mx-1 mt-1.5 text-xs text-red-600">
                                    <X className="h-3.5 w-3.5" /> {saveError}
                                </p>
                            )}

                            {/* Test status */}
                            {testState === 'success' && (
                                <p className="flex items-center gap-1 mx-1 mt-1.5 text-xs text-green-600">
                                    <Check className="h-3.5 w-3.5" /> {testMessage || 'API key is valid'}
                                </p>
                            )}
                            {testState === 'error' && (
                                <p className="flex items-center gap-1 mx-1 mt-1.5 text-xs text-red-600">
                                    <X className="h-3.5 w-3.5" /> {testMessage || 'API key validation failed'}
                                </p>
                            )}
                        </div>
                    )}
                </div>
            </div>
        </div >
    )
}









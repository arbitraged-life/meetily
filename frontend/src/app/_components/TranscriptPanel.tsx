import { VirtualizedTranscriptView } from '@/components/VirtualizedTranscriptView';
import ScreenContextTimeline from '@/components/ScreenContextTimeline';
import { PermissionWarning } from '@/components/PermissionWarning';
import { Button } from '@/components/ui/button';
import { ButtonGroup } from '@/components/ui/button-group';
import { Copy, GlobeIcon, Tag, Sparkles } from 'lucide-react';
import { useTranscripts } from '@/contexts/TranscriptContext';
import { useConfig } from '@/contexts/ConfigContext';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import MeetingNotesPanel from '@/components/MeetingNotesPanel';
import { usePermissionCheck } from '@/hooks/usePermissionCheck';
import { ModalType } from '@/hooks/useModalState';
import { useIsLinux } from '@/hooks/usePlatform';
import { useMemo, useState, useEffect, useRef } from 'react';

/**
 * TranscriptPanel Component
 *
 * Displays transcript content with controls for copying and language settings.
 * Uses TranscriptContext, ConfigContext, and RecordingStateContext internally.
 */

interface TranscriptPanelProps {
  // indicates stop-processing state for transcripts; derived from backend statuses.
  isProcessingStop: boolean;
  isStopping: boolean;
  showModal: (name: ModalType, message?: string) => void;
}

export function TranscriptPanel({
  isProcessingStop,
  isStopping,
  showModal
}: TranscriptPanelProps) {
  // Contexts
  const { transcripts, transcriptContainerRef, copyTranscript } = useTranscripts();
  const { transcriptModelConfig } = useConfig();
  const { isRecording, isPaused } = useRecordingState();
  const { checkPermissions, isChecking, hasSystemAudio, hasMicrophone } = usePermissionCheck();
  const isLinux = useIsLinux();

  // Track recording start time for notes timestamps
  const [recordingStartTime, setRecordingStartTime] = useState<number | null>(null);
  useEffect(() => {
    if (isRecording && !recordingStartTime) {
      setRecordingStartTime(Date.now());
    } else if (!isRecording) {
      setRecordingStartTime(null);
    }
  }, [isRecording]);

  // Convert transcripts to segments for virtualized view
  const segments = useMemo(() =>
    transcripts.map(t => ({
      id: t.id,
      timestamp: t.audio_start_time ?? 0,
      endTime: t.audio_end_time,
      text: t.text,
      confidence: t.confidence,
      speaker_id: t.speaker_id,
      speaker_label: t.speaker_label,
      enhanced: t.enhanced,
      enhancedText: t.enhanced_text,
      enhancementProvider: t.enhancement_provider,
    })),
    [transcripts]
  );

  // Side-by-side AI enhancement view: show original + enhanced columns.
  // Auto-on once any enhancement arrives; user can still toggle it off.
  const hasAnyEnhancement = useMemo(
    () => transcripts.some(t => t.enhanced),
    [transcripts]
  );
  const [showEnhanced, setShowEnhanced] = useState(false);
  const userToggledEnhanced = useRef(false);
  useEffect(() => {
    if (hasAnyEnhancement && !userToggledEnhanced.current) {
      setShowEnhanced(true);
    }
  }, [hasAnyEnhancement]);

  return (
    <div ref={transcriptContainerRef} className="w-full border-r border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-950 flex flex-col overflow-y-auto">
      {/* Title area - Sticky header */}
      <div className="sticky top-0 z-10 bg-white dark:bg-gray-950 p-4 border-gray-200 dark:border-gray-800">
        <div className="flex flex-col space-y-3">
          <div className="flex  flex-col space-y-2">
            <div className="flex justify-center  items-center space-x-2">
              <ButtonGroup>
                {transcripts?.length > 0 && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={copyTranscript}
                    title="Copy Transcript"
                  >
                    <Copy />
                    <span className='hidden md:inline'>
                      Copy
                    </span>
                  </Button>
                )}
                {transcriptModelConfig.provider === "localWhisper" &&
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => showModal('languageSettings')}
                    title="Language"
                  >
                    <GlobeIcon />
                    <span className='hidden md:inline'>
                      Language
                    </span>
                  </Button>
                }
                {transcriptModelConfig.provider === "localWhisper" &&
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => showModal('meetingDomainSettings')}
                    title="Meeting Domain — vocabulary hint for Whisper"
                  >
                    <Tag />
                    <span className='hidden md:inline'>
                      Domain
                    </span>
                  </Button>
                }
                {hasAnyEnhancement && (
                  <Button
                    variant={showEnhanced ? "default" : "outline"}
                    size="sm"
                    onClick={() => {
                      userToggledEnhanced.current = true;
                      setShowEnhanced(v => !v);
                    }}
                    title="Toggle side-by-side AI enhancement"
                  >
                    <Sparkles />
                    <span className='hidden md:inline'>
                      AI
                    </span>
                  </Button>
                )}
              </ButtonGroup>
            </div>
          </div>
        </div>
      </div>

      {/* Permission Warning - Not needed on Linux */}
      {!isRecording && !isChecking && !isLinux && (
        <div className="flex justify-center px-4 pt-4">
          <PermissionWarning
            hasMicrophone={hasMicrophone}
            hasSystemAudio={hasSystemAudio}
            onRecheck={checkPermissions}
            isRechecking={isChecking}
          />
        </div>
      )}

      {/* Transcript content */}
      <div className="pb-20">
        <div className="flex justify-center">
          <div className={showEnhanced ? "w-full max-w-[1400px]" : "w-2/3 max-w-[750px]"}>
            <MeetingNotesPanel
              isRecording={isRecording}
              recordingStartTime={recordingStartTime}
            />
            <VirtualizedTranscriptView
              segments={segments}
              isRecording={isRecording}
              isPaused={isPaused}
              isProcessing={isProcessingStop}
              isStopping={isStopping}
              enableStreaming={isRecording}
              showConfidence={true}
              showEnhanced={showEnhanced}
            />
            <ScreenContextTimeline isRecording={isRecording} />
          </div>
        </div>
      </div>
    </div>
  );
}

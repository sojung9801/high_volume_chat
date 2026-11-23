import { ChatRoomScreen } from '@/features/chat/components/ChatRoomScreen';

type ChatRoomPageProps = {
  params: Promise<{ roomId: string }>;
};

export default async function ChatRoomPage({ params }: ChatRoomPageProps) {
  const { roomId } = await params;

  // 방이 바뀌면 화면 상태(메시지, 연결)를 통째로 새로 만든다.
  return <ChatRoomScreen key={roomId} roomId={roomId} />;
}

import { useState } from "react";
import { Flame, Plus, Settings, Trash2, Users, Code, Palette, ClipboardList, Pencil } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { useSessions } from "@/hooks/useSessions";
import { NewSessionDialog } from "@/components/NewSessionDialog";
import { useSessionStore } from "@/store/sessionStore";

const ROLE_ICONS: Record<string, typeof Users> = {
  pm: Users,
  dev: Code,
  designer: Palette,
  manager: ClipboardList,
};

const ROLE_LABELS: Record<string, string> = {
  pm: "需求方",
  dev: "开发者",
  designer: "设计师",
  manager: "项目经理",
};

function roleLabel(role: string): string {
  return ROLE_LABELS[role] ?? "自定义";
}

function RoleIcon({ role, className }: { role: string; className?: string }) {
  const Icon = ROLE_ICONS[role] ?? Pencil;
  return <Icon className={className} />;
}

interface SidebarProps {
  onOpenSettings: () => void;
}

export function Sidebar({ onOpenSettings }: SidebarProps) {
  const { sessions, currentSessionId, selectSession, deleteSession } = useSessions();
  const [newDialogOpen, setNewDialogOpen] = useState(false);
  const isLoading = useSessionStore((s) => s.isLoading);

  return (
    <aside className="flex h-full w-[260px] flex-col border-r bg-card">
      <div className="flex items-center gap-2 px-4 py-4">
        <Flame className="h-6 w-6 text-orange-500" />
        <span className="text-lg font-bold">Grill-Me</span>
      </div>
      <Separator />
      <ScrollArea className="flex-1">
        <nav className="flex flex-col gap-1 p-2">
          {isLoading && sessions.length === 0 && (
            <div className="px-2 py-4 text-sm text-muted-foreground">加载中...</div>
          )}
          {!isLoading && sessions.length === 0 && (
            <div className="px-2 py-4 text-sm text-muted-foreground">暂无会话</div>
          )}
          {sessions.map((session) => (
            <div
              key={session.id}
              className={cn(
                "group flex cursor-pointer items-center justify-between rounded-md px-3 py-2 text-sm transition-colors hover:bg-accent",
                currentSessionId === session.id && "bg-accent"
              )}
              onClick={() => selectSession(session.id)}
            >
              <div className="flex min-w-0 flex-1 items-start gap-2">
                <RoleIcon role={session.role} className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <span className="truncate font-medium">{session.title}</span>
                  <div className="flex items-center gap-1.5">
                    <Badge variant="outline" className="text-[10px]">
                      {roleLabel(session.role)}
                    </Badge>
                    <Badge
                      variant={session.status === "completed" ? "secondary" : "default"}
                      className="text-[10px]"
                    >
                      {session.status === "completed" ? "已完成" : "进行中"}
                    </Badge>
                  </div>
                </div>
              </div>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7 shrink-0 opacity-0 group-hover:opacity-100"
                onClick={(e) => {
                  e.stopPropagation();
                  deleteSession(session.id);
                }}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>
            </div>
          ))}
        </nav>
      </ScrollArea>
      <Separator />
      <div className="flex items-center justify-between p-2">
        <Button
          variant="ghost"
          size="icon"
          onClick={onOpenSettings}
          title="设置"
        >
          <Settings className="h-5 w-5" />
        </Button>
        <Button
          className="flex-1"
          size="sm"
          onClick={() => setNewDialogOpen(true)}
        >
          <Plus className="mr-1 h-4 w-4" />
          新建会话
        </Button>
      </div>
      <NewSessionDialog open={newDialogOpen} onOpenChange={setNewDialogOpen} />
    </aside>
  );
}

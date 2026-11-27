import { useEffect, useRef } from "react";
import { SYSTEM_CENTER_TABS, type SystemCenterTabId } from "../../types/navigation";
import { useSystemCenter } from "../../hooks/useSystemCenter";
import { ArrowLeftIcon } from "./SystemCenterIcons";
import { OverviewTab } from "./OverviewTab";
import { HealthTab } from "./HealthTab";
import { UpdatesTab } from "./UpdatesTab";
import { SystemInfoTab } from "./SystemInfoTab";
import { AboutTab } from "./AboutTab";

interface SystemCenterPageProps {
  appVersion: string;
  activeTab: SystemCenterTabId;
  onSelectTab: (tab: SystemCenterTabId) => void;
  onBackToDashboard: () => void;
}

export function SystemCenterPage({
  appVersion,
  activeTab,
  onSelectTab,
  onBackToDashboard,
}: SystemCenterPageProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const mainEl = containerRef.current?.closest("main");
    if (mainEl) {
      mainEl.scrollTop = 0;
    }
  }, [activeTab]);

  const {
    telemetry,
    uptimeSeconds,
    healthReport,
    healthCached,
    healthRefreshing,
    healthCheckedAtMs,
    healthError,
    refreshHealth,
    updateResult,
    updatesCached,
    updatesChecking,
    updatesCheckedAtMs,
    checkUpdates,
    systemInfo,
    systemInfoLoading,
    activity,
  } = useSystemCenter(appVersion, activeTab);

  return (
    <div ref={containerRef} className="flex min-h-full w-full min-w-0 flex-col bg-[#0a0c10]">
      {/* Persistent System Center Header Bar with Centered Pill Navigation */}
      <div className="sticky top-0 z-20 border-b border-[#181d26] bg-[#0a0c10]/95 px-4 py-3 backdrop-blur-xs lg:px-6">
        <div className="mx-auto flex w-full max-w-[1160px] items-center justify-between gap-2">
          {/* Left: Return to Dashboard + Title */}
          <div className="flex shrink-0 items-center gap-2.5">
            <button
              type="button"
              onClick={onBackToDashboard}
              title="Return to Dashboard"
              aria-label="Return to Dashboard"
              className="inline-flex cursor-pointer items-center gap-1 rounded-full border border-[#222834] bg-[#12151c] px-2.5 py-1 text-[11.5px] font-medium text-[#94a3b8] transition-colors hover:border-[#323b4c] hover:bg-[#191e28] hover:text-white focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
            >
              <ArrowLeftIcon className="h-3.5 w-3.5" />
              <span className="hidden xl:inline">Dashboard</span>
            </button>

            <span className="whitespace-nowrap text-[15px] font-semibold tracking-tight text-white">
              UXF System Center
            </span>
          </div>

          {/* Center: 5-Section Persistent Pill Navigation */}
          <nav
            aria-label="System Center sections"
            className="flex shrink-0 items-center gap-0.5 rounded-full border border-[#202531] bg-[#11141b] p-1"
          >
            {SYSTEM_CENTER_TABS.map((tab) => {
              const isSelected = tab.id === activeTab;
              const showHealthDot =
                tab.id === "health" && healthReport?.overall_status === "failed";
              const showWarnDot =
                tab.id === "health" && healthReport?.overall_status === "warning";
              const showUpdateDot =
                tab.id === "updates" && Boolean(updateResult?.update_available);

              return (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => onSelectTab(tab.id)}
                  aria-current={isSelected ? "page" : undefined}
                  className={`inline-flex cursor-pointer items-center gap-1.5 whitespace-nowrap rounded-full px-3 py-1 text-[12px] font-medium transition-all duration-150 focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff] xl:px-4 xl:py-1.5 xl:text-[12.5px] ${
                    isSelected
                      ? "bg-[#222732] font-semibold text-white shadow-xs"
                      : "text-[#94a3b8] hover:text-slate-100"
                  }`}
                >
                  <span>{tab.label}</span>
                  {showHealthDot && (
                    <span className="h-1.5 w-1.5 rounded-full bg-red-500" />
                  )}
                  {showWarnDot && (
                    <span className="h-1.5 w-1.5 rounded-full bg-amber-400" />
                  )}
                  {showUpdateDot && (
                    <span className="h-1.5 w-1.5 rounded-full bg-[#7dd3fc]" />
                  )}
                </button>
              );
            })}
          </nav>

          {/* Right: Version tag */}
          <div className="hidden shrink-0 justify-end sm:flex">
            <span className="font-mono text-[11px] text-[#6b7280]">
              v{appVersion.replace(/^v/i, "")}
            </span>
          </div>
        </div>
      </div>

      {/* Main Content Area */}
      <div className="mx-auto w-full max-w-[1160px] flex-1 px-4 py-4 lg:px-6 lg:py-5">
        {activeTab === "overview" && (
          <OverviewTab
            appVersion={appVersion}
            uptimeSeconds={uptimeSeconds}
            telemetry={telemetry}
            healthReport={healthReport}
            healthCached={healthCached}
            healthRefreshing={healthRefreshing}
            updateResult={updateResult}
            updatesChecking={updatesChecking}
            systemInfo={systemInfo}
            activity={activity}
            onNavigateTab={onSelectTab}
          />
        )}

        {activeTab === "health" && (
          <HealthTab
            report={healthReport}
            cached={healthCached}
            refreshing={healthRefreshing}
            checkedAtMs={healthCheckedAtMs}
            error={healthError}
            onRefresh={() => void refreshHealth(true)}
          />
        )}

        {activeTab === "updates" && (
          <UpdatesTab
            appVersion={appVersion}
            updateResult={updateResult}
            cached={updatesCached}
            checking={updatesChecking}
            checkedAtMs={updatesCheckedAtMs}
            onCheckAgain={() => void checkUpdates(true)}
          />
        )}

        {activeTab === "system-info" && (
          <SystemInfoTab
            info={systemInfo}
            loading={systemInfoLoading}
            appVersion={appVersion}
          />
        )}

        {activeTab === "about" && (
          <AboutTab
            appVersion={appVersion}
            healthReport={healthReport}
            systemInfo={systemInfo}
          />
        )}
      </div>
    </div>
  );
}

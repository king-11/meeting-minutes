import Sidebar from '@/components/Sidebar'
import { SidebarProvider } from '@/components/Sidebar/SidebarProvider'
import MainContent from '@/components/MainContent'
import AnalyticsProvider from '@/components/AnalyticsProvider'

export default function MainLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <AnalyticsProvider>
      <SidebarProvider>
        <div className="titlebar h-8 w-full fixed top-0 left-0 bg-transparent" />
        <div className="flex">
          <Sidebar />
          <MainContent>{children}</MainContent>
        </div>
      </SidebarProvider>
    </AnalyticsProvider>
  )
}
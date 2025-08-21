export default function FloatingLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <div style={{ 
      margin: 0, 
      padding: 0, 
      overflow: 'hidden',
      backgroundColor: 'transparent',
      width: '100%',
      height: '100%'
    }}>
      {children}
    </div>
  )
}
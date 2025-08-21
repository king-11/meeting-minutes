import './globals.css'
import { Source_Sans_3 } from 'next/font/google'

const sourceSans3 = Source_Sans_3({ 
  subsets: ['latin'],
  weight: ['400', '500', '600', '700'],
  variable: '--font-source-sans-3',
})

export { metadata } from './metadata'

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="en">
      <body className={`${sourceSans3.variable} font-sans`}>
        {children}
      </body>
    </html>
  )
}

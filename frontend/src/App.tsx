import { useState } from 'react'
import Dashboard from './components/Dashboard'
import './App.css'

function App() {
  const [currentTab, setCurrentTab] = useState<'public' | 'training'>('public')

  return (
    <div className="app">
      <Dashboard mode={currentTab} onTabChange={setCurrentTab} />
    </div>
  )
}

export default App
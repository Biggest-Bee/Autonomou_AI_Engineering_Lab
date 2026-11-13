import React, { useState } from 'react';
import MonacoEditor from 'react-monaco-editor';
import './IDE.css';

interface IDEProps {
  height?: string;
}

const IDE: React.FC<IDEProps> = ({ height = '100%' }) => {
  const [code, setCode] = useState<string>('// Welcome to Autonomous AI Engineering Lab\n// Start coding here...\n\nfunction main() {\n    console.log("Hello, Autonomous AI!");\n    return true;\n}');
  const [language, setLanguage] = useState<string>('typescript');
  const [fileName] = useState<string>('main.ts');

  const handleEditorChange = (newValue: string | undefined) => {
    if (newValue !== undefined) {
      setCode(newValue);
    }
  };

  return (
    <div className="ide-container" style={{ height }}>
      <div className="ide-header">
        <div className="file-tabs">
          <div className="tab active">{fileName}</div>
          <div className="tab">+</div>
        </div>
        <div className="ide-controls">
          <select 
            value={language} 
            onChange={(e) => setLanguage(e.target.value)}
            className="language-selector"
          >
            <option value="typescript">TypeScript</option>
            <option value="javascript">JavaScript</option>
            <option value="python">Python</option>
            <option value="rust">Rust</option>
            <option value="csharp">C#</option>
            <option value="cpp">C++</option>
            <option value="c">C</option>
            <option value="sql">SQL</option>
            <option value="html">HTML</option>
            <option value="css">CSS</option>
          </select>
          <button className="run-button">▶ Run</button>
          <button className="save-button">💾 Save</button>
        </div>
      </div>
      <div className="ide-editor">
        <MonacoEditor
          height="100%"
          language={language}
          value={code}
          onChange={handleEditorChange}
          theme="vs-dark"
          options={{
            minimap: { enabled: true },
            fontSize: 14,
            lineNumbers: 'on',
            roundedSelection: false,
            scrollBeyondLastLine: false,
            automaticLayout: true,
            tabSize: 4,
            wordWrap: 'on',
          }}
        />
      </div>
      <div className="ide-footer">
        <div className="status-bar">
          <span className="status-item">Ln 1, Col 1</span>
          <span className="status-item">UTF-8</span>
          <span className="status-item">TypeScript React</span>
          <span className="status-item">Spaces: 4</span>
        </div>
      </div>
    </div>
  );
};

export default IDE;
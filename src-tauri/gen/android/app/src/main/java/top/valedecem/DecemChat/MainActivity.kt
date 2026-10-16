package top.valedecem.DecemChat

import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.updatePadding

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // edge-to-edge 之下必须自己处理 inset（系统的 adjustResize 不再自动规避）：
    // 不上这一层，内容会顶到状态栏与导航栏底下，软键盘弹出时输入框会被盖住。
    // 把 systemBars + ime 的内边距加到内容视图上，WebView 就会落在安全区内、
    // 并在键盘弹出时被顶上去。
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.ime()
      )
      view.updatePadding(bars.left, bars.top, bars.right, bars.bottom)
      insets
    }
  }
}

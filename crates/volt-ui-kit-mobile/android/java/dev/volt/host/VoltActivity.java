package dev.volt.host;

import android.app.Activity;
import android.content.res.Configuration;
import android.graphics.Color;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.Looper;
import android.text.Editable;
import android.text.TextWatcher;
import android.view.*;
import android.view.accessibility.*;
import android.view.inputmethod.InputMethodManager;
import android.widget.*;
import org.json.*;
import java.nio.file.Files;
import java.nio.file.Paths;

/** Shared OS adapter, not app screens. Views/state/callbacks live in Rust. */
public final class VoltActivity extends Activity implements SurfaceHolder.Callback2 {
    static { System.loadLibrary("volt_app"); }
    private static native int nativeVersion();
    private static native long nativeCreate(byte[] font);
    private static native String nativeAttach(long id, Surface surface);
    private static native void nativeDetach(long id);
    private static native String nativeEvent(long id, String event);
    private static native void nativeDestroy(long id);
    private static native String nativeError();
    private FrameLayout root;
    private CanvasView canvas;
    private EditText editor;
    private TextView diagnostic;
    private long host;
    private boolean attached, failed, paused=true, updating, dragging;
    private JSONArray nodes=new JSONArray();
    private JSONObject snapshot=new JSONObject();
    private String editingId=null, pressedId=null;
    private float startX,startY,scale=1;
    private int retries;
    private final Runnable retry=()->send("draw");

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        root=new FrameLayout(this);canvas=new CanvasView();editor=new EditText(this);
        diagnostic=new TextView(this);diagnostic.setTextSize(14);diagnostic.setPadding(24,24,24,24);
        root.addView(canvas,new FrameLayout.LayoutParams(-1,-1));
        root.addView(diagnostic,new FrameLayout.LayoutParams(-1,-2));
        diagnostic.setVisibility(View.GONE);setContentView(root);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        getWindow().setDecorFitsSystemWindows(false);
        root.setOnApplyWindowInsetsListener((view,insets)->{
            android.graphics.Insets bars=insets.getInsets(WindowInsets.Type.systemBars()|WindowInsets.Type.displayCutout());
            android.graphics.Insets ime=insets.getInsets(WindowInsets.Type.ime());
            view.setPadding(bars.left,bars.top,bars.right,Math.max(bars.bottom,ime.bottom));return insets;
        });
        editor.setSingleLine(true);editor.setTextSize(17);
        editor.setInputType(android.text.InputType.TYPE_CLASS_TEXT|android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        editor.setImeOptions(android.view.inputmethod.EditorInfo.IME_ACTION_DONE);
        editor.setOnEditorActionListener((v,action,event)->{done();return true;});
        editor.addTextChangedListener(new TextWatcher(){
            public void beforeTextChanged(CharSequence s,int start,int count,int after){}
            public void onTextChanged(CharSequence s,int start,int before,int count){}
            public void afterTextChanged(Editable value){
                // Keep composing text native until the IME commits it.
                if (!updating && editingId!=null && android.view.inputmethod.BaseInputConnection.getComposingSpanStart(value)<0)
                    send("text","id",editingId,"text",value.toString());
            }
        });
        canvas.getHolder().setFormat(android.graphics.PixelFormat.OPAQUE);
        canvas.getHolder().addCallback(this);
        canvas.setOnTouchListener((view,event)->touch(event));
        try {
            if(nativeVersion()!=1)throw new IllegalStateException("Rust/Android host ABI mismatch");
            byte[] font=Files.readAllBytes(Paths.get("/system/fonts/Roboto-Regular.ttf"));
            host=nativeCreate(font);if(host==0)throw new IllegalStateException(nativeError());
        } catch(Exception e){error(e.toString());}
    }
    @Override protected void onResume(){super.onResume();paused=false;send("appearance","dark",dark());send("suspend","value",false);}
    @Override protected void onPause(){done();paused=true;send("suspend","value",true);root.removeCallbacks(retry);super.onPause();}
    @Override protected void onDestroy(){root.removeCallbacks(retry);if(host!=0){nativeDestroy(host);host=0;}super.onDestroy();}
    @Override public void onConfigurationChanged(Configuration configuration){super.onConfigurationChanged(configuration);send("appearance","dark",dark());}
    @Override public void onBackPressed(){
        if(editingId!=null){done();return;}
        if(snapshot.optBoolean("canGoBack"))send("back");else super.onBackPressed();
    }
    private boolean dark(){return (getResources().getConfiguration().uiMode&Configuration.UI_MODE_NIGHT_MASK)==Configuration.UI_MODE_NIGHT_YES;}
    @Override public void surfaceCreated(SurfaceHolder holder){
        if(host==0||failed)return;
        attached=refresh(nativeAttach(host,holder.getSurface()));resize();
    }
    @Override public void surfaceRedrawNeeded(SurfaceHolder holder){resize();if(attached)send("draw");}
    @Override public void surfaceChanged(SurfaceHolder holder,int format,int width,int height){resize();}
    @Override public void surfaceDestroyed(SurfaceHolder holder){
        root.removeCallbacks(retry);attached=false;if(host!=0)nativeDetach(host);
    }
    private void resize(){
        if(!attached||canvas.getWidth()<1||canvas.getHeight()<1)return;
        scale=Math.max(.25f,Math.min(4f,getResources().getDisplayMetrics().density));
        send("resize","width",canvas.getWidth()/scale,"height",canvas.getHeight()/scale,"scale",scale);
    }
    private boolean send(String op,Object...values){
        if(host==0||failed)return false;
        if(Looper.myLooper()!=Looper.getMainLooper())throw new IllegalStateException("Not the main Looper");
        try{
            JSONObject event=new JSONObject().put("op",op);
            for(int i=0;i<values.length;i+=2)event.put((String)values[i],values[i+1]);
            return refresh(nativeEvent(host,event.toString()));
        }catch(Exception e){error(e.toString());return false;}
    }
    private boolean refresh(String json){
        try{
            if(json==null)throw new IllegalStateException("Rust returned no snapshot");
            JSONObject next=new JSONObject(json);
            if(next.has("error"))throw new IllegalStateException(next.getString("error"));
            snapshot=next;nodes=next.getJSONArray("nodes");
            int mask=WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
            if(getWindow().getInsetsController()!=null)getWindow().getInsetsController().setSystemBarsAppearance(next.optBoolean("dark")?0:mask,mask);
            root.setBackgroundColor(color(next.getJSONArray("background")));
            editor.setTextColor(color(next.getJSONArray("foreground")));
            editor.setBackgroundColor(color(next.getJSONArray("fieldBackground")));
            // A one-line read-only render witness, also useful to QA without logs.
            canvas.setContentDescription("Volt renderer: "+next.optString("backend")+"; frames="+next.optLong("frames"));
            positionEditor();canvas.sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
            root.removeCallbacks(retry);
            if(next.optInt("status")==2 && attached && !paused){
                if(++retries>3)throw new IllegalStateException("Vulkan surface did not recover after three retries");
                root.postDelayed(retry,50);
            }else if(next.optInt("status")==1)retries=0;
            return true;
        }catch(Exception e){error(e.toString());return false;}
    }
    private static int color(JSONArray rgba)throws JSONException{return Color.argb(Math.round((float)rgba.getDouble(3)*255),Math.round((float)rgba.getDouble(0)*255),Math.round((float)rgba.getDouble(1)*255),Math.round((float)rgba.getDouble(2)*255));}
    private void error(String message){failed=true;diagnostic.setText("Volt mobile host\n\n"+message);diagnostic.setTextColor(Color.RED);diagnostic.setBackgroundColor(Color.WHITE);diagnostic.setVisibility(View.VISIBLE);android.util.Log.e("VoltHost",message);}
    private Rect rect(JSONObject node,String key){
        JSONArray b=node.optJSONArray(key);if(b==null||b.length()!=4)return new Rect();
        return new Rect(Math.round((float)b.optDouble(0)*scale),Math.round((float)b.optDouble(1)*scale),Math.round((float)(b.optDouble(0)+b.optDouble(2))*scale),Math.round((float)(b.optDouble(1)+b.optDouble(3))*scale));
    }
    private JSONObject find(String id){for(int i=0;i<nodes.length();i++){JSONObject n=nodes.optJSONObject(i);if(n!=null&&n.optString("id").equals(id))return n;}return null;}
    private JSONObject at(float x,float y,String kind){
        for(int i=nodes.length()-1;i>=0;i--){JSONObject n=nodes.optJSONObject(i);if(n!=null&&n.optBoolean("enabled")&&n.optString("kind").equals(kind)&&rect(n,"visibleBounds").contains((int)x,(int)y)&&(!kind.equals("field")||!n.optBoolean("readOnly")))return n;}return null;
    }
    private void begin(JSONObject node){
        String id=node.optString("id");if(!id.equals(editingId))done();editingId=id;
        send("focus","id",id);updating=true;editor.setText(node.optString("value",""));editor.setSelection(editor.length());updating=false;
        editor.setContentDescription(node.optString("label"));
        if(editor.getParent()==null)root.addView(editor);
        positionEditor();editor.requestFocus();editor.post(()->((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).showSoftInput(editor,InputMethodManager.SHOW_IMPLICIT));
    }
    private void positionEditor(){
        if(editingId==null)return;JSONObject node=find(editingId);if(node==null){done();return;}
        Rect r=rect(node,"bounds");FrameLayout.LayoutParams p=new FrameLayout.LayoutParams(Math.max(1,r.width()),Math.max(1,r.height()));
        p.leftMargin=r.left;p.topMargin=r.top;editor.setLayoutParams(p);
    }
    private void done(){
        if(editingId==null)return;String id=editingId;editingId=null;
        String text=editor.getText().toString();editor.clearFocus();
        ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(editor.getWindowToken(),0);
        root.removeView(editor);send("text","id",id,"text",text);
    }
    private boolean activate(JSONObject n){
        if(!n.optBoolean("enabled"))return false;
        if(n.optString("kind").equals("field")){if(n.optBoolean("readOnly"))return false;begin(n);return true;}
        done();return send("activate","id",n.optString("id"));
    }
    private boolean touch(MotionEvent e){
        if(paused||failed||!attached)return false;
        float x=e.getX(),y=e.getY();
        switch(e.getActionMasked()){
        case MotionEvent.ACTION_DOWN:
            startX=x;startY=y;dragging=false;JSONObject first=at(x,y,"button");pressedId=first==null?null:first.optString("id");send("begin","x",x/scale,"y",y/scale);return true;
        case MotionEvent.ACTION_MOVE:
            dragging=dragging||Math.hypot(x-startX,y-startY)>=8*scale;send("move","x",x/scale,"y",y/scale);return true;
        case MotionEvent.ACTION_UP:
            dragging=dragging||Math.hypot(x-startX,y-startY)>=8*scale;
            JSONObject button=dragging?null:at(x,y,"button"),field=dragging?null:at(x,y,"field");
            if(editingId!=null&&button!=null&&button.optString("id").equals(pressedId)){send("cancel");done();send("activate","id",button.optString("id"));return true;}
            if(field==null)done();send("end","x",x/scale,"y",y/scale);if(field!=null)begin(field);return true;
        case MotionEvent.ACTION_CANCEL:send("cancel");return true;
        default:return true;
        }
    }
    private final class CanvasView extends SurfaceView {
        CanvasView(){super(VoltActivity.this);setFocusable(true);setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_YES);}
        @Override protected void onSizeChanged(int w,int h,int oldw,int oldh){super.onSizeChanged(w,h,oldw,oldh);resize();}
        @Override public AccessibilityNodeProvider getAccessibilityNodeProvider(){return provider;}
        private final AccessibilityNodeProvider provider=new AccessibilityNodeProvider(){
            @Override public AccessibilityNodeInfo createAccessibilityNodeInfo(int id){
                if(id==View.NO_ID){AccessibilityNodeInfo info=AccessibilityNodeInfo.obtain(CanvasView.this);onInitializeAccessibilityNodeInfo(info);for(int i=0;i<nodes.length();i++){JSONObject n=nodes.optJSONObject(i);if(n!=null&&!rect(n,"visibleBounds").isEmpty()&&!n.optString("label").isEmpty()&&!n.optString("kind").equals("scroll"))info.addChild(CanvasView.this,i+1);}return info;}
                JSONObject n=nodes.optJSONObject(id-1);if(n==null)return null;
                AccessibilityNodeInfo info=AccessibilityNodeInfo.obtain();info.setSource(CanvasView.this,id);info.setParent(CanvasView.this);info.setPackageName(getPackageName());
                String kind=n.optString("kind");info.setClassName(kind.equals("button")||kind.equals("field")?"android.widget.Button":"android.widget.TextView");info.setText(n.optString("label"));info.setContentDescription(n.optString("id"));info.setEnabled(n.optBoolean("enabled"));
                Rect r=rect(n,"visibleBounds");info.setBoundsInParent(r);int[] origin=new int[2];CanvasView.this.getLocationOnScreen(origin);r.offset(origin[0],origin[1]);info.setBoundsInScreen(r);info.setVisibleToUser(!r.isEmpty());
                if(kind.equals("button")||kind.equals("field")){info.setClickable(!n.optBoolean("readOnly"));info.addAction(AccessibilityNodeInfo.ACTION_CLICK);}return info;
            }
            @Override public boolean performAction(int id,int action,Bundle args){JSONObject n=nodes.optJSONObject(id-1);if(n!=null&&action==AccessibilityNodeInfo.ACTION_CLICK)return activate(n);return false;}
        };
    }
}

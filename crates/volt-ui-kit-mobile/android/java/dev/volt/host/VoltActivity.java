package dev.volt.host;

import android.app.Activity;
import android.content.res.Configuration;
import android.graphics.Color;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
import android.view.*;
import android.view.accessibility.*;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
import android.widget.*;
import org.json.*;
import java.lang.ref.WeakReference;
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

    /** Changes from Rust worker threads: apply them on the main Looper. */
    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static WeakReference<VoltActivity> current = new WeakReference<>(null);
    @SuppressWarnings("unused") // Called from Rust.
    static void wake(long id) {
        MAIN.post(() -> { VoltActivity a = current.get(); if (a != null && a.host == id) a.send("updates"); });
    }

    private FrameLayout root;
    private CanvasView canvas;
    private EditText editor;
    private TextView diagnostic;
    private long host;
    private boolean attached, failed, paused=true, updating, dragging, held, framing;
    private JSONArray nodes=new JSONArray();
    private JSONObject snapshot=new JSONObject();
    private String editingId=null, pressedId=null, fieldAtStart=null, lastEditing=null;
    private float startX,startY,scale=1;
    private int retries;
    private long holds;
    private final Runnable retry=()->send("draw");
    /** The finger rested in place: ask Rust for the menu under it. */
    private final Runnable hold=()->{
        if(dragging||editingId!=null)return;
        long before=holds;send("hold","x",startX/scale,"y",startY/scale);
        if(holds!=before){held=true;canvas.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS);}
    };
    /** One frame while Rust animates. */
    private final Choreographer.FrameCallback frame=new Choreographer.FrameCallback(){
        public void doFrame(long nanos){
            framing=false;
            if(paused||failed||!attached)return;
            send("draw");
            if(snapshot.optBoolean("animating"))schedule();
        }
    };
    private void schedule(){if(!framing){framing=true;Choreographer.getInstance().postFrameCallback(frame);}}

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        current=new WeakReference<>(this);
        root=new FrameLayout(this);canvas=new CanvasView();editor=new EditText(this);
        diagnostic=new TextView(this);diagnostic.setTextSize(14);diagnostic.setPadding(24,24,24,24);
        root.addView(canvas,new FrameLayout.LayoutParams(-1,-1));
        root.addView(diagnostic,new FrameLayout.LayoutParams(-1,-2));
        diagnostic.setVisibility(View.GONE);setContentView(root);
        // Edge to edge: Rust lays out under the bars and keeps clear of them.
        getWindow().setDecorFitsSystemWindows(false);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING);
        root.setOnApplyWindowInsetsListener((view,insets)->{
            android.graphics.Insets bars=insets.getInsets(WindowInsets.Type.systemBars()|WindowInsets.Type.displayCutout());
            android.graphics.Insets ime=insets.getInsets(WindowInsets.Type.ime());
            send("safe_area","top",bars.top/scale,"right",bars.right/scale,"bottom",bars.bottom/scale,"left",bars.left/scale);
            // A dock or message box rides the keyboard; sheets rise above it.
            send("bars","bottom",Math.max(bars.bottom,ime.bottom)/scale,"keyboard",ime.bottom/scale);
            positionEditor();
            return insets;
        });
        editor.setTextSize(17);editor.setBackground(null);
        editor.setPadding(Math.round(14*getResources().getDisplayMetrics().density),0,Math.round(14*getResources().getDisplayMetrics().density),0);
        editor.setOnEditorActionListener((v,action,event)->{
            String id=editingId;
            if(action==EditorInfo.IME_ACTION_SEARCH&&id!=null){send("text","id",id,"text",editor.getText().toString());send("submit","id",id);}
            done();return true;
        });
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
            if(nativeVersion()!=3)throw new IllegalStateException("Rust/Android host ABI mismatch");
            byte[] font=Files.readAllBytes(Paths.get("/system/fonts/Roboto-Regular.ttf"));
            host=nativeCreate(font);if(host==0)throw new IllegalStateException(nativeError());
        } catch(Exception e){error(e.toString());}
    }
    @Override protected void onResume(){super.onResume();paused=false;sendAppearance();send("suspend","value",false);send("updates");}
    @Override protected void onPause(){done();paused=true;send("suspend","value",true);root.removeCallbacks(retry);super.onPause();}
    @Override protected void onDestroy(){root.removeCallbacks(retry);if(host!=0){nativeDestroy(host);host=0;}super.onDestroy();}
    @Override public void onConfigurationChanged(Configuration configuration){super.onConfigurationChanged(configuration);sendAppearance();}
    @Override public void onBackPressed(){
        if(editingId!=null){done();return;}
        if(snapshot.optBoolean("canGoBack"))send("back");else super.onBackPressed();
    }
    private void sendAppearance(){
        float animation=android.provider.Settings.Global.getFloat(getContentResolver(),android.provider.Settings.Global.ANIMATOR_DURATION_SCALE,1f);
        send("appearance","dark",dark(),"reduce_motion",animation==0f);
        // TalkBack on or off: messages that close themselves wait for it.
        AccessibilityManager access=(AccessibilityManager)getSystemService(ACCESSIBILITY_SERVICE);
        if(access!=null){
            if(!watchingReader){watchingReader=true;access.addTouchExplorationStateChangeListener(on->send("screen_reader","on",on));}
            send("screen_reader","on",access.isTouchExplorationEnabled());
        }
    }
    private boolean watchingReader;
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
        root.requestApplyInsets();
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
            holds=next.optLong("holds",holds);
            int mask=WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS|WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
            if(getWindow().getInsetsController()!=null)getWindow().getInsetsController().setSystemBarsAppearance(next.optBoolean("dark")?0:mask,mask);
            root.setBackgroundColor(color(next.getJSONArray("background")));
            editor.setTextColor(color(next.getJSONArray("foreground")));
            // A one-line read-only render witness, also useful to QA without logs.
            canvas.setContentDescription("Volt renderer: "+next.optString("backend")+"; frames="+next.optLong("frames"));
            // If an editor node went away (a closed search), put the keyboard away.
            if(editingId!=null&&find(editingId)==null)done();
            positionEditor();canvas.sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
            // Rust asked for a field to be typed in (a search that opened):
            // open the keyboard once, when the request changes.
            String editing=next.isNull("editing")?null:next.optString("editing",null);
            if(editing!=null?!editing.equals(lastEditing):lastEditing!=null){
                lastEditing=editing;
                if(editing!=null&&!editing.equals(editingId)){
                    final String want=editing;
                    MAIN.post(()->{JSONObject n=find(want);if(n!=null&&want.equals(lastEditing)&&!want.equals(editingId)&&!n.optBoolean("readOnly"))begin(n);});
                }
            }
            if(next.optBoolean("animating")&&!paused)schedule();
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
    private static boolean editable(JSONObject n){String k=n.optString("kind");return k.equals("field")||k.equals("search");}
    private JSONObject at(float x,float y,String kind){
        for(int i=nodes.length()-1;i>=0;i--){
            JSONObject n=nodes.optJSONObject(i);if(n==null||!n.optBoolean("enabled")||!rect(n,"visibleBounds").contains((int)x,(int)y))continue;
            if(kind.equals("field")?editable(n)&&!n.optBoolean("readOnly"):n.optString("kind").equals(kind))return n;
        }return null;
    }
    /** The keyboard and suggestions for what the field holds. */
    private void applyInputKind(JSONObject node){
        String kind=node.optString("inputKind","text");boolean multiline=node.optBoolean("multiline");
        int type;
        switch(kind){
            case "name":type=InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_PERSON_NAME|InputType.TYPE_TEXT_FLAG_CAP_WORDS;break;
            case "email":type=InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS;break;
            case "url":type=InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_URI;break;
            case "number":case "one_time_code":type=InputType.TYPE_CLASS_NUMBER;break;
            case "decimal":type=InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_FLAG_DECIMAL;break;
            case "phone":type=InputType.TYPE_CLASS_PHONE;break;
            case "password":case "secret":type=InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_PASSWORD;break;
            default:type=InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_FLAG_CAP_SENTENCES|(multiline?InputType.TYPE_TEXT_FLAG_MULTI_LINE:0);
        }
        if(node.optBoolean("password"))type=InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_PASSWORD;
        editor.setInputType(type);
        editor.setSingleLine(!multiline);
        if(multiline){editor.setMaxLines(12);editor.setGravity(Gravity.TOP|Gravity.START);}else editor.setGravity(Gravity.CENTER_VERTICAL|Gravity.START);
        if(kind.equals("one_time_code"))editor.setAutofillHints("smsOTPCode"); // AndroidX HintConstants.AUTOFILL_HINT_SMS_OTP
        else if(kind.equals("email"))editor.setAutofillHints(View.AUTOFILL_HINT_EMAIL_ADDRESS);
        else if(kind.equals("password"))editor.setAutofillHints(View.AUTOFILL_HINT_PASSWORD);
        else editor.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_AUTO);
        boolean search=node.optString("kind").equals("search")||node.optBoolean("search");
        editor.setImeOptions(search?EditorInfo.IME_ACTION_SEARCH:(multiline?EditorInfo.IME_ACTION_NONE:EditorInfo.IME_ACTION_DONE));
    }
    private void begin(JSONObject node){
        String id=node.optString("id");if(!id.equals(editingId))done();editingId=id;
        send("focus","id",id);updating=true;applyInputKind(node);editor.setText(node.optString("value",""));editor.setSelection(editor.length());updating=false;
        editor.setContentDescription(node.optString("label"));
        // Blend in with what's behind the field: its colour and its corners.
        JSONObject backdrop=node.optJSONObject("backdrop");
        try{
            android.graphics.drawable.GradientDrawable fill=new android.graphics.drawable.GradientDrawable();
            fill.setColor(backdrop!=null?color(backdrop.getJSONArray("color")):color(snapshot.getJSONArray("fieldBackground")));
            fill.setCornerRadius(backdrop!=null?(float)backdrop.optDouble("radius",8)*scale:8*scale);
            editor.setBackground(fill);
        }catch(JSONException e){editor.setBackground(null);}
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
        root.removeView(editor);send("text","id",id,"text",text);send("blur");
    }
    private boolean activate(JSONObject n){
        if(!n.optBoolean("enabled"))return false;
        if(editable(n)){if(n.optBoolean("readOnly"))return false;begin(n);return true;}
        done();return send("activate","id",n.optString("id"));
    }
    private boolean touch(MotionEvent e){
        if(paused||failed||!attached)return false;
        float x=e.getX(),y=e.getY();
        switch(e.getActionMasked()){
        case MotionEvent.ACTION_DOWN:{
            startX=x;startY=y;dragging=false;held=false;
            JSONObject first=at(x,y,"button");pressedId=first==null?null:first.optString("id");
            // The field under the finger now: opening the keyboard can move it.
            JSONObject field=at(x,y,"field");fieldAtStart=field==null?null:field.optString("id");
            send("begin","x",x/scale,"y",y/scale);
            root.removeCallbacks(hold);root.postDelayed(hold,450);
            return true;}
        case MotionEvent.ACTION_MOVE:
            dragging=dragging||Math.hypot(x-startX,y-startY)>=8*scale;
            if(dragging)root.removeCallbacks(hold);
            if(!held)send("move","x",x/scale,"y",y/scale);return true;
        case MotionEvent.ACTION_UP:{
            root.removeCallbacks(hold);
            if(held){held=false;return true;}
            dragging=dragging||Math.hypot(x-startX,y-startY)>=8*scale;
            JSONObject button=dragging?null:at(x,y,"button");
            JSONObject field=dragging?null:(fieldAtStart!=null?find(fieldAtStart):at(x,y,"field"));
            if(editingId!=null&&button!=null&&button.optString("id").equals(pressedId)){
                send("cancel");
                if(button.optBoolean("keepsKeyboard")){
                    // Send in a message box: commit, press, keep typing.
                    String id=editingId;send("text","id",id,"text",editor.getText().toString());
                    send("activate","id",button.optString("id"));
                    JSONObject n=find(id);if(n==null){done();return true;}
                    send("focus","id",id);updating=true;editor.setText(n.optString("value",""));editor.setSelection(editor.length());updating=false;
                    return true;
                }
                done();send("activate","id",button.optString("id"));return true;
            }
            if(field==null)done();send("end","x",x/scale,"y",y/scale);if(field!=null)begin(field);return true;}
        case MotionEvent.ACTION_CANCEL:root.removeCallbacks(hold);send("cancel");return true;
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
                String kind=n.optString("kind");boolean control=kind.equals("button")||editable(n);
                info.setClassName(editable(n)?"android.widget.EditText":control?"android.widget.Button":"android.widget.TextView");info.setText(n.optString("label"));info.setContentDescription(n.optString("id"));info.setEnabled(n.optBoolean("enabled"));
                Rect r=rect(n,"visibleBounds");info.setBoundsInParent(r);int[] origin=new int[2];CanvasView.this.getLocationOnScreen(origin);r.offset(origin[0],origin[1]);info.setBoundsInScreen(r);info.setVisibleToUser(!r.isEmpty());
                if(control){info.setClickable(!n.optBoolean("readOnly"));info.addAction(AccessibilityNodeInfo.ACTION_CLICK);}
                // A row's swipe actions and menu, by name, in TalkBack's actions menu.
                JSONArray actions=n.optJSONArray("actions");
                for(int a=0;actions!=null&&a<actions.length();a++){
                    JSONObject action=actions.optJSONObject(a);if(action==null)continue;
                    info.addAction(new AccessibilityNodeInfo.AccessibilityAction(ACTION_BASE+a,action.optString("label")));
                }
                return info;
            }
            @Override public boolean performAction(int id,int action,Bundle args){
                JSONObject n=nodes.optJSONObject(id-1);if(n==null)return false;
                if(action==AccessibilityNodeInfo.ACTION_CLICK)return activate(n);
                JSONArray actions=n.optJSONArray("actions");int index=action-ACTION_BASE;
                if(actions!=null&&index>=0&&index<actions.length()){
                    JSONObject a=actions.optJSONObject(index);if(a==null)return false;
                    if(a.isNull("target"))return send("menu","id",n.optString("id"));
                    return send("activate","id",a.optString("target"));
                }
                return false;
            }
        };
    }
    /** Custom accessibility action IDs: well clear of the framework's. */
    private static final int ACTION_BASE=0x7f0f0000;
}
